#[macro_export]
macro_rules! map {
    () => {
        $crate::vec_map::VecMap::new()
    };
    (@count $k1:expr, $v1:expr; $($($k:expr, $v:expr;)+)?) => {
        1 $( + map!(@count $( $k, $v; )*) )?
    };
    ($($k:expr => $v:expr),+ $(,)?) => {{
        let mut new = $crate::vec_map::VecMap::with_capacity(map!(@count $($k,$v;)+));
        $(
            let old = new.insert($k, $v);
            debug_assert_eq!(old, None);
        )+
        new
    }};
    ($(($k:expr, $v:expr)),+ $(,)?) => {{
        let mut new = $crate::vec_map::VecMap::with_capacity(map!(@count $( $k, $v; )+));
        $(
            let old = new.insert($k, $v);
            debug_assert_eq!(old, None);
        )+
        new
    }};
    ($k:ty => $v:ty; $cap:expr) => {
        $crate::vec_map::VecMap::<$k, $v>::with_capacity($cap)
    };
    (($k:ty, $v:ty); $cap:expr) => {
        $crate::vec_map::VecMap::<$k, $v>::with_capacity($cap)
    };
}
