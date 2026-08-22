use snafu::prelude::*;

#[test]
fn self_in_source_field() {
    #[derive(Debug, PartialEq, Snafu)]
    enum Error {
        NonRecursive,
        Recursive {
            #[snafu(source(from(Error, Box::new)))]
            source: Box<Self>,
        },
    }

    let e = NonRecursiveSnafu
        .fail::<i32>()
        .context(RecursiveSnafu)
        .unwrap_err();

    assert_eq!(
        e,
        Error::Recursive {
            source: Box::new(Error::NonRecursive)
        },
    );
}

#[test]
fn self_in_implicit_field() {
    #[derive(Debug, PartialEq)]
    struct ImplicitType<T>(T);
    impl snafu::GenerateImplicitData for ImplicitType<Box<Error>> {
        fn generate() -> Self {
            Self(Box::new(Error::NonRecursive { value: 123 }))
        }
    }
    #[derive(Debug, PartialEq, Snafu)]
    enum Error {
        NonRecursive {
            value: i32,
        },
        Recursive {
            #[snafu(implicit)]
            a_field: ImplicitType<Box<Self>>,
        },
    }

    let e = RecursiveSnafu.build();

    assert_eq!(
        e,
        Error::Recursive {
            a_field: ImplicitType(Box::new(Error::NonRecursive { value: 123 })),
        },
    );
}

#[test]
fn self_in_user_field() {
    #[derive(Debug, PartialEq, Snafu)]
    enum Error {
        NonRecursive { value: i32 },
        Recursive { other: Vec<Self> },
    }

    let e = RecursiveSnafu {
        other: vec![
            NonRecursiveSnafu { value: 1 }.build(),
            NonRecursiveSnafu { value: 2 }.build(),
        ],
    }
    .build();

    assert_eq!(
        e,
        Error::Recursive {
            other: vec![
                Error::NonRecursive { value: 1 },
                Error::NonRecursive { value: 2 },
            ],
        },
    );
}
