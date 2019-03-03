use crate::tile::Tile;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;
use std::time::Duration;
use indexmap::IndexMap;

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

    fn dump(&self, current_name: &str, depth: usize, parent_duration: Duration) {
        let to_float = |d: Duration| d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
        let percentage = 100.0 * to_float(self.duration) / to_float(parent_duration);
        let real_name = if current_name.is_empty() {
            "TOTAL"
        } else {
            current_name
        };
        eprintln!("{}{}: {:.2}% ({:.3?})", "\t".repeat(depth), real_name, percentage, self.duration);
        for (child_name, child) in self.children.iter() {
            child.borrow().dump(child_name, depth + 1, self.duration);
        }
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

pub struct PerfStats {
    tile: Tile,
    root_element: PerfStatsElementRef,
    element_stack: PerfStatsElementStackRef,
    start_time: Instant,
}

impl PerfStats {
    pub fn new(tile: Tile) -> PerfStats {
        let root = PerfStatsElement::create();
        PerfStats {
            tile,
            root_element: Rc::clone(&root),
            element_stack: Rc::new(RefCell::new(vec![Rc::clone(&root)])),
            start_time: Instant::now(),
        }
    }

    pub fn measure(&self, name: impl Into<String>) -> Measurer {
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

    pub fn dump(&self) {
        eprintln!("Stats for tile {:?}", self.tile);
        let root_duration = Instant::now() - self.start_time;
        self.root_element.borrow_mut().duration = root_duration;
        self.root_element.borrow().dump("", 0, root_duration);
    }
}
