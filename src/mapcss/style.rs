use std::hash::{Hash, Hasher};

use mapcss::color::Color;
use mapcss::styler::LineCap;

fn optional_float_to_int(float: &Option<f64>) -> Option<u64> {
    float.map(|x| x.to_bits())
}

fn optional_float_vec_to_int(vec: &Option<Vec<f64>>) -> Option<Vec<u64>> {
    vec.as_ref()
        .map(|x| x.iter().map(|y| y.to_bits()).collect::<Vec<_>>())
}

macro_rules! define_style_fields {
    ($($id:ident : $type:ty,)*) => (
        #[derive(Clone, Debug)]
        pub struct Style {
            $(pub $id: $type,)*
        }
    );
}

macro_rules! define_style_hash {
    ($self:ident, $state:ident,) => ();

    ($self:ident, $state:ident, $id:ident : f64, $($rest:tt)*) => (
        $self.$id.to_bits().hash($state);
        define_style_hash!($self, $state, $($rest)*);
    );

    ($self:ident, $state:ident, $id:ident : Option<f64>, $($rest:tt)*) => (
        optional_float_to_int(&$self.$id).hash($state);
        define_style_hash!($self, $state, $($rest)*);
    );

    ($self:ident, $state:ident, $id:ident : Option<Vec<f64>>, $($rest:tt)*) => (
        optional_float_vec_to_int(&$self.$id).hash($state);
        define_style_hash!($self, $state, $($rest)*);
    );

    ($self:ident, $state:ident, $id:ident : $type:ty, $($rest:tt)*) => (
        $self.$id.hash($state);
        define_style_hash!($self, $state, $($rest)*);
    );
}

macro_rules! define_style_eq {
    ($self:ident, $other:ident,) => ();

    ($self:ident, $other:ident, $id:ident : f64, $($rest:tt)*) => (
        if $self.$id.to_bits() != $other.$id.to_bits() {
            return false;
        }
        define_style_eq!($self, $other, $($rest)*);
    );

    ($self:ident, $other:ident, $id:ident : Option<f64>, $($rest:tt)*) => (
        if optional_float_to_int(&$self.$id) != optional_float_to_int(&$other.$id) {
            return false;
        }
        define_style_eq!($self, $other, $($rest)*);
    );

    ($self:ident, $other:ident, $id:ident : Option<Vec<f64>>, $($rest:tt)*) => (
        if optional_float_vec_to_int(&$self.$id) != optional_float_vec_to_int(&$other.$id) {
            return false;
        }
        define_style_eq!($self, $other, $($rest)*);
    );

    ($self:ident, $other:ident, $id:ident : $type:ty, $($rest:tt)*) => (
        if $self.$id != $other.$id {
            return false;
        }
        define_style_eq!($self, $other, $($rest)*);
    );
}

macro_rules! define_style {
    ($($rest:tt)*) => (
        define_style_fields!($($rest)*);

        impl Hash for Style {
            fn hash<H: Hasher>(&self, state: &mut H) {
                define_style_hash!(self, state, $($rest)*);
            }
        }

        impl PartialEq for Style {
            fn eq(&self, other: &Style) -> bool {
                define_style_eq!(self, other, $($rest)*);
                return true;
            }
        }

        impl Eq for Style {}
    );
}

define_style! {
    z_index: f64,

    color: Option<Color>,
    fill_color: Option<Color>,
    is_foreground_fill: bool,
    background_color: Option<Color>,
    opacity: Option<f64>,
    fill_opacity: Option<f64>,

    width: Option<f64>,
    dashes: Option<Vec<f64>>,
    line_cap: Option<LineCap>,

    casing_color: Option<Color>,
    casing_width: Option<f64>,
    casing_dashes: Option<Vec<f64>>,
    casing_line_cap: Option<LineCap>,

    icon_image: Option<String>,
}
