impl crate::quick::quick_law::QuickLaw {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Commutative => "commutative",
            Self::Associative => "associative",
            Self::Identity(_) => "identity",
            Self::SelfInverse(_) => "self-inverse",
            Self::Idempotent => "idempotent",
            Self::Involution => "involution",
        }
    }

    pub(crate) fn recommendation(self) -> String {
        match self {
            Self::Commutative => "Add: AlgebraicLaw::Commutative".to_string(),
            Self::Associative => "Add: AlgebraicLaw::Associative".to_string(),
            Self::Identity(element) => {
                format!("Add: AlgebraicLaw::Identity {{ element: 0x{element:08X} }}")
            }
            Self::SelfInverse(result) => {
                format!("Add: AlgebraicLaw::SelfInverse {{ result: {result} }}")
            }
            Self::Idempotent => "Add: AlgebraicLaw::Idempotent".to_string(),
            Self::Involution => "Add: AlgebraicLaw::Involution".to_string(),
        }
    }
}
