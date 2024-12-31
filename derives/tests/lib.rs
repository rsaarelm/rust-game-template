use bitflags::bitflags;
use derives::*;

bitflags! {
    #[derive(Debug, Eq, PartialEq, SerializeFlags, DeserializeFlags)]
    pub struct TestFlags: u8 {
        const A = 1 << 0;
        const B = 1 << 1;
    }
}

#[test]
fn test_derive_serialize_flags() {
    // Test that empty flags serialize to "-".
    let flags = TestFlags::from_bits_truncate(0);
    assert_eq!(idm::to_string(&flags).unwrap(), "-");
    assert_eq!(idm::from_str::<TestFlags>("-").unwrap(), TestFlags::empty());

    assert_eq!(idm::to_string(&TestFlags::A).unwrap(), "a");
    assert_eq!(idm::from_str::<TestFlags>("a").unwrap(), TestFlags::A);
    assert_eq!(idm::from_str::<TestFlags>("b").unwrap(), TestFlags::B);

    assert_eq!(
        idm::to_string(&(TestFlags::A | TestFlags::B)).unwrap(),
        "a b"
    );
    assert_eq!(
        idm::from_str::<TestFlags>("a b").unwrap(),
        TestFlags::A | TestFlags::B
    );
    assert_eq!(
        idm::from_str::<TestFlags>("b a").unwrap(),
        TestFlags::A | TestFlags::B
    );
}

#[test]
fn test_tail_flags() {
    type Row = (i32, i32, TestFlags);

    #[derive(Debug, Eq, PartialEq, serde::Deserialize)]
    struct Row2 {
        x: i32,
        y: i32,
        flags: TestFlags,
    }

    assert_eq!(
        idm::from_str::<Row>("1 2 -").unwrap(),
        (1, 2, TestFlags::empty())
    );
    assert_eq!(
        idm::from_str::<Row2>("1 2 -").unwrap(),
        Row2 {
            x: 1,
            y: 2,
            flags: TestFlags::empty()
        }
    );

    assert_eq!(
        idm::from_str::<Row2>("1 2 a").unwrap(),
        Row2 {
            x: 1,
            y: 2,
            flags: TestFlags::A
        }
    );
    assert_eq!(
        idm::from_str::<Row2>("1 2 a b").unwrap(),
        Row2 {
            x: 1,
            y: 2,
            flags: TestFlags::A | TestFlags::B
        }
    );
}
