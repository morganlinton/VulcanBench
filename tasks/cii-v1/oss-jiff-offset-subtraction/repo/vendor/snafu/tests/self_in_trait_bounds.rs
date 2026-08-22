#[test]
fn self_in_enum_bounds() {
    #[derive(Debug, PartialEq, snafu::Snafu)]
    enum Error
    where
        i32: From<Self>,
    {
        Variant { value: i32 },
    }
    impl From<Error> for i32 {
        fn from(value: Error) -> Self {
            let Error::Variant { value } = value;
            value
        }
    }

    let e = VariantSnafu { value: 1 }.build();

    assert_eq!(e, Error::Variant { value: 1 });
    assert_eq!(Into::<i32>::into(e), 1);
}

#[test]
fn self_in_struct_bounds() {
    #[derive(Debug, PartialEq, snafu::Snafu)]
    struct Error
    where
        i32: From<Self>,
    {
        value: i32,
    }
    impl From<Error> for i32 {
        fn from(value: Error) -> Self {
            value.value
        }
    }

    let e = Snafu { value: 1 }.build();

    assert_eq!(e, Error { value: 1 });
    assert_eq!(Into::<i32>::into(e), 1);
}
