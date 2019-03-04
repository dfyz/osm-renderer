use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;
use std::time::Duration;
use indexmap::IndexMap;
use std::collections::BTreeMap;

struct PerfStatsElement {
    duration: Duration,
    children: IndexMap<String, Rc<RefCell<PerfStatsElement>>>,
}

type PerfStatsElementRef = Rc<RefCell<PerfStatsElement>>;
type PerfStatsElementStackRef = Rc<RefCell<Vec<PerfStatsElementRef>>>;

impl PerfStatsElement {
    fn create() -> PerfStatsElementRef {
        Rc::new(RefCell::new(PerfStatsElement {
            duration: Duration::default(),
            children: IndexMap::new(),
        }))
    }
}

pub struct Measurer {
    start_time: Instant,
    element: PerfStatsElementRef,
    element_stack: PerfStatsElementStackRef,
}

impl Drop for Measurer {
    fn drop(&mut self) {
        self.element.borrow_mut().duration += Instant::now() - self.start_time;
        self.element_stack.borrow_mut().pop();
    }
}

struct TilePerfStats {
    zoom: u8,
    root_element: PerfStatsElementRef,
    element_stack: PerfStatsElementStackRef,
    start_time: Instant,
}

impl TilePerfStats {
    fn new(zoom: u8) -> TilePerfStats {
        let root = PerfStatsElement::create();
        TilePerfStats {
            zoom,
            root_element: Rc::clone(&root),
            element_stack: Rc::new(RefCell::new(vec![Rc::clone(&root)])),
            start_time: Instant::now(),
        }
    }

    fn measure(&self, name: impl Into<String>) -> Measurer {
        let mut stack = self.element_stack.borrow_mut();

        let new_element = {
            let name = name.into();
            let mut current_element = stack.last().unwrap().borrow_mut();

            if let Some(existing_child) = current_element.children.get(&name) {
                Rc::clone(existing_child)
            } else {
                let new_child = PerfStatsElement::create();
                current_element.children.insert(name, Rc::clone(&new_child));
                new_child
            }
        };

        stack.push(Rc::clone(&new_element));

        Measurer {
            start_time: Instant::now(),
            element: Rc::clone(&new_element),
            element_stack: Rc::clone(&self.element_stack),
        }
    }

    fn finalize(&mut self) {
        self.root_element.borrow_mut().duration = Instant::now() - self.start_time;
    }
}

#[derive(Default)]
struct SummedPerfStatsElement {
    duration_sum: Duration,
    children: IndexMap<String, Box<SummedPerfStatsElement>>,
}

impl SummedPerfStatsElement {
    fn add(&mut self, element: &PerfStatsElementRef) {
        self.duration_sum += element.borrow().duration;
        for (other_child_name, other_child) in element.borrow().children.iter() {
            if let Some(our_child) = self.children.get_mut(other_child_name) {
                our_child.add(other_child);
            } else {
                let mut new_child = Box::new(SummedPerfStatsElement::default());
                new_child.add(other_child);
                self.children.insert(other_child_name.clone(), new_child);
            }
        }
    }
}

#[derive(Default)]
struct SummedPerfStats {
    root_element: SummedPerfStatsElement,
    count: u32,
}

#[derive(Default)]
pub struct PerfStats {
    stats_by_zoom: BTreeMap<u8, SummedPerfStats>,
}

impl PerfStats {
    fn add_tile_stats(&mut self, tile_stats: TilePerfStats) {
        let mut zoom_stats = self.stats_by_zoom.entry(tile_stats.zoom).or_default();
        zoom_stats.root_element.add(&tile_stats.root_element);
        zoom_stats.count += 1;

        self.dump();
    }

    pub fn dump(&self) {
        for (zoom, zoom_stats) in self.stats_by_zoom.iter() {
            eprintln!("ZOOM {} ({} tiles)", zoom, zoom_stats.count);
            eprintln!("=======");
            dump_summed_perf_stats_element("", &zoom_stats.root_element, 0, None, zoom_stats.count);
        }
    }
}

fn dump_summed_perf_stats_element(current_name: &str, current_element: &SummedPerfStatsElement, depth: usize, parent_duration: Option<Duration>, duration_count: u32) {
    let normalized_duration = current_element.duration_sum / duration_count;
    let to_float = |d: Duration| d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
    let percentage = 100.0 * if let Some(parent_duration) = parent_duration {
        to_float(normalized_duration) / to_float(parent_duration)
    } else {
        1.0
    };
    let real_name = if current_name.is_empty() {
        "TOTAL"
    } else {
        current_name
    };
    eprintln!("{}{}: {:.2}% ({:.3?})", "\t".repeat(depth), real_name, percentage, normalized_duration);
    for (child_name, child) in current_element.children.iter() {
        dump_summed_perf_stats_element(child_name, child, depth + 1, Some(normalized_duration), duration_count);
    }
}

thread_local!(static TLS_PERF_STATS: RefCell<Option<TilePerfStats>> = RefCell::new(None));

pub fn start_tile(zoom: u8) {
    TLS_PERF_STATS.with(|stats| stats.borrow_mut().replace(TilePerfStats::new(zoom)));
}

pub fn finish_tile(total_stats: &mut PerfStats) {
    TLS_PERF_STATS.with(|stats| {
        let mut tile_stats = stats.borrow_mut().take().unwrap();
        tile_stats.finalize();
        total_stats.add_tile_stats(tile_stats);
    });
}

pub fn measure(name: impl Into<String>) -> Measurer {
    TLS_PERF_STATS.with(|stats| stats.borrow_mut().as_mut().unwrap().measure(name))
}
