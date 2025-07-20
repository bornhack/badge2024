// The `static_cell` crate also contains a version of this macro
// that has support for attributes and also does not require you to specify
// the type, however it also requires using a nightly compiler
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}
