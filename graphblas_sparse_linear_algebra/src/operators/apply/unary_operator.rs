use std::marker::PhantomData;
use std::ptr;

use crate::collections::sparse_matrix::{GraphblasSparseMatrixTrait, SparseMatrix};
use crate::collections::sparse_vector::{GraphblasSparseVectorTrait, SparseVector};
use crate::context::{CallGraphBlasContext, ContextTrait};
use crate::error::SparseLinearAlgebraError;
use crate::operators::binary_operator::AccumulatorBinaryOperator;
use crate::operators::{
    binary_operator::BinaryOperator, options::OperatorOptions, unary_operator::UnaryOperator,
};
use crate::value_type::{AsBoolean, ValueType};

use crate::bindings_to_graphblas_implementation::{
    GrB_BinaryOp, GrB_Descriptor, GrB_Matrix_apply, GrB_UnaryOp, GrB_Vector_apply,
};

// Implemented methods do not provide mutable access to GraphBLAS operators or options.
// Code review must consider that no mtable access is provided.
// https://doc.rust-lang.org/nomicon/send-and-sync.html
unsafe impl<EvaluationDomain: ValueType> Send
    for UnaryOperatorApplier<EvaluationDomain>
{
}
unsafe impl<EvaluationDomain: ValueType> Sync
    for UnaryOperatorApplier<EvaluationDomain>
{
}

#[derive(Debug, Clone)]
pub struct UnaryOperatorApplier<
    EvaluationDomain: ValueType,
> {
    _evaluation_domain: PhantomData<EvaluationDomain>,

    unary_operator: GrB_UnaryOp,
    accumulator: GrB_BinaryOp,
    options: GrB_Descriptor,
}

impl<EvaluationDomain: ValueType>
    UnaryOperatorApplier<EvaluationDomain>
{
    pub fn new(
        unary_operator: &impl UnaryOperator<EvaluationDomain>,
        options: &OperatorOptions,
        accumulator: &impl AccumulatorBinaryOperator<EvaluationDomain>,
    ) -> Self {
        Self {
            unary_operator: unary_operator.graphblas_type(),
            accumulator: accumulator.accumulator_graphblas_type(),
            options: options.to_graphblas_descriptor(),

            _evaluation_domain: PhantomData,
        }
    }

    pub(crate) unsafe fn unary_operator(&self) -> GrB_UnaryOp {
        self.unary_operator
    }
    pub(crate) unsafe fn accumulator(&self) -> GrB_BinaryOp {
        self.accumulator
    }
    pub(crate) unsafe fn options(&self) -> GrB_Descriptor {
        self.options
    }
}

pub trait ApplyUnaryOperator<EvaluationDomain>
where
    EvaluationDomain: ValueType,
{
    fn apply_to_vector(
        &self,
        argument: &(impl GraphblasSparseVectorTrait + ContextTrait),
        product: &mut (impl GraphblasSparseVectorTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError>;

    fn apply_to_vector_with_mask(
        &self,
        argument: &(impl GraphblasSparseVectorTrait + ContextTrait),
        product: &mut (impl GraphblasSparseVectorTrait + ContextTrait),
        mask: &(impl GraphblasSparseVectorTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError>;

    fn apply_to_matrix(
        &self,
        argument: &(impl GraphblasSparseMatrixTrait + ContextTrait),
        product: &mut (impl GraphblasSparseMatrixTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError>;

    fn apply_to_matrix_with_mask(
        &self,
        argument: &(impl GraphblasSparseMatrixTrait + ContextTrait),
        product: &mut (impl GraphblasSparseMatrixTrait + ContextTrait),
        mask: &(impl GraphblasSparseMatrixTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError>;
}

impl<EvaluationDomain: ValueType>
    ApplyUnaryOperator<EvaluationDomain>
    for UnaryOperatorApplier<EvaluationDomain>
{
    fn apply_to_vector(
        &self,
        argument: &(impl GraphblasSparseVectorTrait + ContextTrait),
        product: &mut (impl GraphblasSparseVectorTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = argument.context();

        context.call(
            || unsafe {
                GrB_Vector_apply(
                    product.graphblas_vector(),
                    ptr::null_mut(),
                    self.accumulator,
                    self.unary_operator,
                    argument.graphblas_vector(),
                    self.options,
                )
            },
            unsafe { &product.graphblas_vector() },
        )?;

        Ok(())
    }

    fn apply_to_vector_with_mask(
        &self,
        argument: &(impl GraphblasSparseVectorTrait + ContextTrait),
        product: &mut (impl GraphblasSparseVectorTrait + ContextTrait),
        mask: &(impl GraphblasSparseVectorTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = argument.context();

        context.call(
            || unsafe {
                GrB_Vector_apply(
                    product.graphblas_vector(),
                    mask.graphblas_vector(),
                    self.accumulator,
                    self.unary_operator,
                    argument.graphblas_vector(),
                    self.options,
                )
            },
            unsafe { &product.graphblas_vector() },
        )?;

        Ok(())
    }

    fn apply_to_matrix(
        &self,
        argument: &(impl GraphblasSparseMatrixTrait + ContextTrait),
        product: &mut (impl GraphblasSparseMatrixTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = argument.context();

        context.call(
            || unsafe {
                GrB_Matrix_apply(
                    product.graphblas_matrix(),
                    ptr::null_mut(),
                    self.accumulator,
                    self.unary_operator,
                    argument.graphblas_matrix(),
                    self.options,
                )
            },
            unsafe { &product.graphblas_matrix() },
        )?;

        Ok(())
    }

    fn apply_to_matrix_with_mask(
        &self,
        argument: &(impl GraphblasSparseMatrixTrait + ContextTrait),
        product: &mut (impl GraphblasSparseMatrixTrait + ContextTrait),
        mask: &(impl GraphblasSparseMatrixTrait + ContextTrait),
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = argument.context();

        context.call(
            || unsafe {
                GrB_Matrix_apply(
                    product.graphblas_matrix(),
                    mask.graphblas_matrix(),
                    self.accumulator,
                    self.unary_operator,
                    argument.graphblas_matrix(),
                    self.options,
                )
            },
            unsafe { &product.graphblas_matrix() },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::collections::sparse_matrix::{
        FromMatrixElementList, GetMatrixElementValue, MatrixElementList, Size,
    };
    use crate::collections::sparse_vector::{
        FromVectorElementList, GetVectorElementValue, VectorElementList,
    };
    use crate::collections::Collection;
    use crate::context::{Context, Mode};
    use crate::operators::binary_operator::{Assignment, First};
    use crate::operators::unary_operator::{Identity, LogicalNegation, One};

    #[test]
    fn test_matrix_unary_operator() {
        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let element_list = MatrixElementList::<u8>::from_element_vector(vec![
            (1, 1, 1).into(),
            (2, 1, 2).into(),
            (4, 2, 4).into(),
            (5, 2, 5).into(),
        ]);

        let matrix_size: Size = (10, 15).into();
        let matrix = SparseMatrix::<u8>::from_element_list(
            &context.clone(),
            &matrix_size,
            &element_list,
            &First::<u8>::new(),
        )
        .unwrap();

        let mut product_matrix = SparseMatrix::<u8>::new(&context, &matrix_size).unwrap();

        let operator = UnaryOperatorApplier::new(
            &One::<u8>::new(),
            &OperatorOptions::new_default(),
            &Assignment::<u8>::new(),
        );

        operator
            .apply_to_matrix(&matrix, &mut product_matrix)
            .unwrap();

        println!("{}", product_matrix);

        assert_eq!(product_matrix.number_of_stored_elements().unwrap(), 4);
        assert_eq!(
            product_matrix
                .get_element_value_or_default(&(2, 1).into())
                .unwrap(),
            1
        );
        assert_eq!(
            product_matrix.get_element_value(&(9, 1).into()).unwrap(),
            None
        );

        let operator = UnaryOperatorApplier::new(
            &Identity::<u8>::new(),
            &OperatorOptions::new_default(),
            &Assignment::<u8>::new(),
        );
        operator
            .apply_to_matrix(&matrix, &mut product_matrix)
            .unwrap();

        println!("{}", matrix);
        println!("{}", product_matrix);

        assert_eq!(product_matrix.number_of_stored_elements().unwrap(), 4);
        assert_eq!(
            product_matrix
                .get_element_value_or_default(&(2, 1).into())
                .unwrap(),
            2
        );
        assert_eq!(
            product_matrix.get_element_value(&(9, 1).into()).unwrap(),
            None
        );
    }

    #[test]
    fn test_vector_unary_operator() {
        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let element_list = VectorElementList::<u8>::from_element_vector(vec![
            (1, 1).into(),
            (2, 2).into(),
            (4, 4).into(),
            (5, 5).into(),
        ]);

        let vector_length: usize = 10;
        let vector = SparseVector::<u8>::from_element_list(
            &context.clone(),
            &vector_length,
            &element_list,
            &First::<u8>::new(),
        )
        .unwrap();

        let mut product_vector = SparseVector::<u8>::new(&context, &vector_length).unwrap();

        let operator = UnaryOperatorApplier::new(
            &One::<u8>::new(),
            &OperatorOptions::new_default(),
            &Assignment::<u8>::new(),
        );

        operator
            .apply_to_vector(&vector, &mut product_vector)
            .unwrap();

        println!("{}", product_vector);

        assert_eq!(product_vector.number_of_stored_elements().unwrap(), 4);
        assert_eq!(product_vector.get_element_value_or_default(&2).unwrap(), 1);
        assert_eq!(product_vector.get_element_value(&9).unwrap(), None);

        let operator = UnaryOperatorApplier::new(
            &Identity::<u8>::new(),
            &OperatorOptions::new_default(),
            &Assignment::<u8>::new(),
        );
        operator
            .apply_to_vector(&vector, &mut product_vector)
            .unwrap();

        println!("{}", vector);
        println!("{}", product_vector);

        assert_eq!(product_vector.number_of_stored_elements().unwrap(), 4);
        assert_eq!(product_vector.get_element_value_or_default(&2).unwrap(), 2);
        assert_eq!(product_vector.get_element_value(&9).unwrap(), None);
    }

    #[test]
    fn test_vector_unary_negation_operator() {
        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let vector_length: usize = 10;
        let vector = SparseVector::<bool>::new(&context, &vector_length).unwrap();

        let mut product_vector = SparseVector::<bool>::new(&context, &vector_length).unwrap();

        let operator = UnaryOperatorApplier::new(
            &LogicalNegation::<bool>::new(),
            &OperatorOptions::new_default(),
            &Assignment::<bool>::new(),
        );

        operator
            .apply_to_vector(&vector, &mut product_vector)
            .unwrap();

        println!("{}", product_vector);

        assert_eq!(product_vector.number_of_stored_elements().unwrap(), 0);
    }
}
