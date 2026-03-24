use strum::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum BinaryOperator {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Modulo,
    LogicalAnd,
    LogicalOr,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXOR,
    LeftShift,
    RightShift,
    UnsignedRightShift,
    Equals,
    EqualsStrict,
    GreaterThan,
    GreaterThanOrEqual,
    InstanceOf,
}

impl BinaryOperator {
    pub fn get_operator(&self) -> &'static str {
        match self {
            BinaryOperator::Addition => "+",
            BinaryOperator::Subtraction => "-",
            BinaryOperator::Multiplication => "*",
            BinaryOperator::Division => "/",
            BinaryOperator::Modulo => "%",
            BinaryOperator::LogicalAnd => "&&",
            BinaryOperator::LogicalOr => "||",
            BinaryOperator::BitwiseAnd => "&",
            BinaryOperator::BitwiseOr => "|",
            BinaryOperator::BitwiseXOR => "^",
            BinaryOperator::LeftShift => "<<",
            BinaryOperator::RightShift => ">>",
            BinaryOperator::UnsignedRightShift => ">>>",
            BinaryOperator::Equals => "==",
            BinaryOperator::EqualsStrict => "===",
            BinaryOperator::GreaterThan => ">",
            BinaryOperator::GreaterThanOrEqual => ">=",
            BinaryOperator::InstanceOf => "instanceof",
        }
    }
}
