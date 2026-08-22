use std::marker::PhantomData;

use snafu::prelude::*;

#[test]
fn self_in_source_field() {
    #[derive(Debug, PartialEq, Snafu)]
    struct TerminatingError<T> {
        marker: PhantomData<T>,
    }
    #[derive(Debug, PartialEq, Snafu)]
    struct Error {
        source: TerminatingError<Self>,
    }
    let source: Result<(), _> = TerminatingSnafu {
        marker: PhantomData::default(),
    }
    .fail();

    let e = source.context(Snafu).unwrap_err();

    assert_eq!(
        e,
        Error {
            source: TerminatingError {
                marker: PhantomData::default(),
            }
        },
    );
}

#[test]
fn self_in_implicit_field() {
    #[derive(Debug, PartialEq)]
    struct ImplicitType<T>(T);
    impl snafu::GenerateImplicitData for ImplicitType<Option<Box<Error>>> {
        fn generate() -> Self {
            Self(Some(Box::new(Error { value: Self(None) })))
        }
    }
    #[derive(Debug, PartialEq, Snafu)]
    struct Error {
        // Option is necessary to make this terminate.
        // Box is necessary to make the size finite.
        #[snafu(implicit)]
        value: ImplicitType<Option<Box<Self>>>,
    }

    let e = Snafu.build();

    assert_eq!(
        e,
        Error {
            value: ImplicitType(Some(Box::new(Error {
                value: ImplicitType(None),
            }))),
        },
    );
}

#[test]
fn self_in_user_field() {
    #[derive(Debug, PartialEq, Snafu)]
    struct Error {
        other: Option<Box<Self>>,
    }

    let e = Snafu {
        other: Some(Box::new(Snafu { other: None }.build())),
    }
    .build();

    assert_eq!(
        e,
        Error {
            other: Some(Box::new(Error { other: None })),
        },
    );
}
