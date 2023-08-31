use super::*;
use crate::map;

#[test]
fn split_off() {
    let mut vec = map![1 => "1", 2 => "2", 5 => "5"];

    assert_eq!(vec.split_off(&3), map![5 => "5"]);
    assert_eq!(vec, map![1 => "1", 2 => "2"]);

    assert_eq!(vec.split_off(&0), map![1 => "1", 2 => "2"]);
    assert_eq!(vec, map![]);
}

#[test]
fn merge() {
    let mut a = map![
        2 => "b",
        3 => "c",
        5 => "f",
    ];
    let mut b = map![
        1 => "a",
        3 => "d",
        4 => "e",
    ];
    a.append(&mut b);

    assert_eq!(
        a,
        map![
            1 => "a",
            2 => "b",
            3 => "d",
            4 => "e",
            5 => "f",
        ]
    );
    assert_eq!(b, map![]);

    let mut a = map![
        1 => "a",
        3 => "d",
        5 => "f",
    ];
    let mut b = map![];
    a.append(&mut b);

    assert_eq!(
        a,
        map![
            1 => "a",
            3 => "d",
            5 => "f",
        ]
    );
    assert_eq!(b, map![]);

    let mut a = map![];
    let mut b = map![
        1 => "a",
        3 => "d",
        5 => "f",
    ];
    a.append(&mut b);

    assert_eq!(
        a,
        map![
            1 => "a",
            3 => "d",
            5 => "f",
        ]
    );
    assert_eq!(b, map![]);

    let mut a = map![i32 => i32; 0];
    let mut b = map![];
    a.append(&mut b);
    assert_eq!(a, map![]);
    assert_eq!(b, map![]);
}

#[test]
fn stack_to_heap() {
    let a: VecMap<i32, ()> = map![_ => _; 10];
    let b = [8; 10];
    let c = vec![8; 10];

    println!("{:?}", a.0.as_ptr());
    println!("{:?}", &b[..] as *const _);
    println!("{:?}", c.as_ptr());
}

#[test]
fn range_idx() {
    let a = map![
        0 => 1,
        1 => 4,
        3 => 16,
    ];

    println!("{:?}", a.find(&4));
}

#[test]
fn extend_from() {
    let mut a = map![(2, "b"), (3, "c"), (5, "f")];
    let b = map![(1, "a"), (3, "d"), (4, "e")];

    a.extend(&b);

    assert_eq!(a.len(), 5);
    assert_eq!(b.len(), 3);

    a.extend(b);

    assert_eq!(a.len(), 5);
}

#[test]
fn try_push_last() {
    let mut a = map![
        2 => "b",
        3 => "c",
        5 => "f",
    ];

    assert!(!a.push_unsafe(4, "d"));
    assert_eq!(
        a,
        map![
            2 => "b",
            3 => "c",
            5 => "f",
        ]
    );

    assert!(!a.push_unsafe(5, "d"));
    assert_eq!(
        a,
        map![
            2 => "b",
            3 => "c",
            5 => "f",
        ]
    );

    assert!(a.push_unsafe(6, "d"));
    assert_eq!(
        a,
        map![
            2 => "b",
            3 => "c",
            5 => "f",
            6 => "d",
        ]
    );
}
