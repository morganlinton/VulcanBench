//! Tiny u32 expression builders used everywhere in emit. Each helper
//! appends a single naga Expression and returns its handle.

use naga::{BinaryOperator, Expression, Literal};

use super::BodyBuilder;

impl BodyBuilder<'_> {
    pub(super) fn literal_u32(&mut self, value: u32) -> naga::Handle<Expression> {
        self.append_expr(Expression::Literal(Literal::U32(value)))
    }

    pub(super) fn add_u32(
        &mut self,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        self.append_expr(Expression::Binary {
            op: BinaryOperator::Add,
            left,
            right,
        })
    }

    pub(super) fn sub_u32(
        &mut self,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        self.append_expr(Expression::Binary {
            op: BinaryOperator::Subtract,
            left,
            right,
        })
    }

    pub(super) fn div_u32(
        &mut self,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        self.append_expr(Expression::Binary {
            op: BinaryOperator::Divide,
            left,
            right,
        })
    }

    pub(super) fn lt_u32(
        &mut self,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        self.append_expr(Expression::Binary {
            op: BinaryOperator::Less,
            left,
            right,
        })
    }

    pub(super) fn min_u32(
        &mut self,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        let condition = self.lt_u32(left, right);
        self.append_expr(Expression::Select {
            condition,
            accept: left,
            reject: right,
        })
    }

    pub(super) fn byte_size_to_words(
        &mut self,
        size: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        let three = self.literal_u32(3);
        let four = self.literal_u32(4);
        let rounded = self.add_u32(size, three);
        self.div_u32(rounded, four)
    }
}
