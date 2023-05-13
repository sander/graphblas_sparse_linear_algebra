use std::marker::PhantomData;
use std::ptr;

use crate::collections::sparse_matrix::{GraphblasSparseMatrixTrait, SparseMatrix};
use crate::context::{CallGraphBlasContext, ContextTrait};
use crate::error::SparseLinearAlgebraError;
use crate::operators::binary_operator::{AccumulatorBinaryOperator, BinaryOperator};
use crate::operators::options::OperatorOptions;
use crate::operators::semiring::Semiring;
use crate::value_type::{AsBoolean, ValueType};

use crate::bindings_to_graphblas_implementation::{
    GrB_BinaryOp, GrB_Descriptor, GrB_Semiring, GrB_mxm,
};

// Implemented methods do not provide mutable access to GraphBLAS operators or options.
// Code review must consider that no mtable access is provided.
// https://doc.rust-lang.org/nomicon/send-and-sync.html
unsafe impl<
        FirstArgument: ValueType,
        SecondArgument: ValueType,
        Product: ValueType,
        EvaluationDomain: ValueType,
    > Send
    for MatrixMultiplicationOperator<FirstArgument, SecondArgument, Product, EvaluationDomain>
{
}
unsafe impl<
        FirstArgument: ValueType,
        SecondArgument: ValueType,
        Product: ValueType,
        EvaluationDomain: ValueType,
    > Sync
    for MatrixMultiplicationOperator<FirstArgument, SecondArgument, Product, EvaluationDomain>
{
}

#[derive(Debug, Clone)]
pub struct MatrixMultiplicationOperator<Multiplier, Multiplicant, Product, EvaluationDomain>
where
    Multiplier: ValueType,
    Multiplicant: ValueType,
    Product: ValueType,
    EvaluationDomain: ValueType,
{
    // TODO: review if MatrixMultiplicationOperator really needs these types
    _multiplier: PhantomData<Multiplier>,
    _multiplicant: PhantomData<Multiplicant>,
    _product: PhantomData<Product>,
    _evaluation_domain: PhantomData<EvaluationDomain>,

    // mask: GrB_Matrix,
    accumulator: GrB_BinaryOp,
    semiring: GrB_Semiring, // defines '+' and '*' for A*B (not optional for GrB_mxm)
    options: GrB_Descriptor,
}

impl<Multiplier, Multiplicant, Product, EvaluationDomain>
    MatrixMultiplicationOperator<Multiplier, Multiplicant, Product, EvaluationDomain>
where
    Multiplier: ValueType,
    Multiplicant: ValueType,
    Product: ValueType,
    EvaluationDomain: ValueType,
{
    pub fn new(
        semiring: &impl Semiring<Multiplier, Multiplicant, Product, EvaluationDomain>, // defines '+' and '*' for A*B (not optional for GrB_mxm)
        options: &OperatorOptions,
        accumulator: &impl AccumulatorBinaryOperator<Product>,
    ) -> Self {
        Self {
            accumulator: accumulator.accumulator_graphblas_type(),
            semiring: semiring.graphblas_type(),
            options: options.to_graphblas_descriptor(),

            _multiplier: PhantomData,
            _multiplicant: PhantomData,
            _product: PhantomData,
            _evaluation_domain: PhantomData,
        }
    }
}

pub trait MultiplyMatrices<Multiplier: ValueType, Multiplicant: ValueType, Product: ValueType> {
    // TODO: consider a version where the resulting product matrix is generated in the function body
    fn apply(
        &self,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseMatrix<Multiplicant>,
        product: &mut SparseMatrix<Product>,
    ) -> Result<(), SparseLinearAlgebraError>;

    fn apply_with_mask<MaskValueType: ValueType + AsBoolean>(
        &self,
        mask: &SparseMatrix<MaskValueType>,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseMatrix<Multiplicant>,
        product: &mut SparseMatrix<Product>,
    ) -> Result<(), SparseLinearAlgebraError>;
}

impl<
        Multiplier: ValueType,
        Multiplicant: ValueType,
        Product: ValueType,
        EvaluationDomain: ValueType,
    > MultiplyMatrices<Multiplier, Multiplicant, Product>
    for MatrixMultiplicationOperator<Multiplier, Multiplicant, Product, EvaluationDomain>
{
    // TODO: consider a version where the resulting product matrix is generated in the function body
    fn apply(
        &self,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseMatrix<Multiplicant>,
        product: &mut SparseMatrix<Product>,
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = product.context();

        context.call(
            || unsafe {
                GrB_mxm(
                    product.graphblas_matrix(),
                    ptr::null_mut(),
                    self.accumulator,
                    self.semiring,
                    multiplier.graphblas_matrix(),
                    multiplicant.graphblas_matrix(),
                    self.options,
                )
            },
            unsafe { product.graphblas_matrix_ref() },
        )?;

        Ok(())
    }

    fn apply_with_mask<MaskValueType: ValueType + AsBoolean>(
        &self,
        mask: &SparseMatrix<MaskValueType>,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseMatrix<Multiplicant>,
        product: &mut SparseMatrix<Product>,
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = product.context();

        context.call(
            || unsafe {
                GrB_mxm(
                    product.graphblas_matrix(),
                    mask.graphblas_matrix(),
                    self.accumulator,
                    self.semiring,
                    multiplier.graphblas_matrix(),
                    multiplicant.graphblas_matrix(),
                    self.options,
                )
            },
            unsafe { product.graphblas_matrix_ref() },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::collections::sparse_matrix::{
        FromMatrixElementList, GetMatrixElementList, GetMatrixElementValue, MatrixElementList, Size,
    };
    use crate::collections::Collection;
    use crate::context::{Context, Mode};
    use crate::operators::binary_operator::{Assignment, First};
    use crate::operators::binary_operator::{Plus, Times};
    use crate::operators::semiring::PlusTimes;

    #[test]
    fn create_matrix_multiplier() {
        let semiring = PlusTimes::<i64, i64, i64, i64>::new();
        let options = OperatorOptions::new_default();
        let _matrix_multiplier = MatrixMultiplicationOperator::<i64, i64, i64, i64>::new(
            &semiring,
            &options,
            &Assignment::new(),
        );

        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let target_height = 10;
        let target_width = 5;
        let size: Size = (target_height, target_width).into();

        let _mask = SparseMatrix::<f32>::new(&context, &size).unwrap();

        let accumulator = Times::<i64>::new();

        let _matrix_multiplier = MatrixMultiplicationOperator::<i64, i64, i64, i64>::new(
            &semiring,
            &options,
            &accumulator,
        );
    }

    #[test]
    fn test_multiplication_with_plus_times() {
        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let semiring = PlusTimes::<f32, f32, f32, f32>::new();
        let options = OperatorOptions::new_default();
        let matrix_multiplier = MatrixMultiplicationOperator::<f32, f32, f32, f32>::new(
            &semiring,
            &options,
            &Assignment::new(),
        );

        let height = 2;
        let width = 2;
        let size: Size = (height, width).into();

        let multiplier = SparseMatrix::<f32>::new(&context, &size).unwrap();
        let multiplicant = multiplier.clone();
        let mut product = multiplier.clone();

        // Test multiplication of empty matrices
        matrix_multiplier
            .apply(&multiplier, &multiplicant, &mut product)
            .unwrap();
        let element_list = product.get_element_list().unwrap();

        assert_eq!(product.number_of_stored_elements().unwrap(), 0);
        assert_eq!(element_list.length(), 0);
        assert_eq!(product.get_element_value(&(1, 1).into()).unwrap(), None); // NoValue

        let multiplier_element_list = MatrixElementList::<f32>::from_element_vector(vec![
            (0, 0, 1.0).into(),
            (1, 0, 2.0).into(),
            (0, 1, 3.0).into(),
            (1, 1, 4.0).into(),
        ]);
        let multiplier = SparseMatrix::<f32>::from_element_list(
            &context,
            &size,
            &multiplier_element_list,
            &First::<f32>::new(),
        )
        .unwrap();

        let multiplicant_element_list = MatrixElementList::<f32>::from_element_vector(vec![
            (0, 0, 5.0).into(),
            (1, 0, 6.0).into(),
            (0, 1, 7.0).into(),
            (1, 1, 8.0).into(),
        ]);
        let multiplicant = SparseMatrix::<f32>::from_element_list(
            &context,
            &size,
            &multiplicant_element_list,
            &First::<f32>::new(),
        )
        .unwrap();

        // Test multiplication of full matrices
        matrix_multiplier
            .apply(&multiplier, &multiplicant, &mut product)
            .unwrap();

        assert_eq!(
            product
                .get_element_value_or_default(&(0, 0).into())
                .unwrap(),
            23.
        );
        assert_eq!(
            product
                .get_element_value_or_default(&(1, 0).into())
                .unwrap(),
            34.
        );
        assert_eq!(
            product
                .get_element_value_or_default(&(0, 1).into())
                .unwrap(),
            31.
        );
        assert_eq!(
            product
                .get_element_value_or_default(&(1, 1).into())
                .unwrap(),
            46.
        );

        // TODO: this test is not generic over column/row storage format.
        // Equality checks should be done at a matrix level, since the ordering of the element list is not guaranteed.
        let expected_product = MatrixElementList::<f32>::from_element_vector(vec![
            (0, 0, 23.).into(),
            (0, 1, 31.).into(),
            (1, 0, 34.).into(),
            (1, 1, 46.).into(),
        ]);
        let product_element_list = product.get_element_list().unwrap();
        assert_eq!(expected_product, product_element_list);

        // test the use of an accumulator
        let accumulator = Plus::<f32>::new();
        let matrix_multiplier_with_accumulator =
            MatrixMultiplicationOperator::<f32, f32, f32, f32>::new(
                &semiring,
                &options,
                &accumulator,
            );

        matrix_multiplier_with_accumulator
            .apply(&multiplier, &multiplicant, &mut product)
            .unwrap();

        assert_eq!(
            product
                .get_element_value_or_default(&(0, 0).into())
                .unwrap(),
            23. * 2.
        );
        assert_eq!(
            product
                .get_element_value_or_default(&(1, 0).into())
                .unwrap(),
            34. * 2.
        );
        assert_eq!(
            product
                .get_element_value_or_default(&(0, 1).into())
                .unwrap(),
            31. * 2.
        );
        assert_eq!(
            product
                .get_element_value_or_default(&(1, 1).into())
                .unwrap(),
            46. * 2.
        );

        // test the use of a mask
        let mask_element_list = MatrixElementList::<u8>::from_element_vector(vec![
            (0, 0, 3).into(),
            (1, 0, 0).into(),
            (1, 1, 1).into(),
        ]);
        let mask = SparseMatrix::<u8>::from_element_list(
            &context,
            &size,
            &mask_element_list,
            &First::<u8>::new(),
        )
        .unwrap();

        let matrix_multiplier = MatrixMultiplicationOperator::<f32, f32, f32, f32>::new(
            &semiring,
            &options,
            &Assignment::new(),
        );

        let mut product = SparseMatrix::<f32>::new(&context, &size).unwrap();

        matrix_multiplier
            .apply_with_mask(&mask, &multiplier, &multiplicant, &mut product)
            .unwrap();

        assert_eq!(
            product
                .get_element_value_or_default(&(0, 0).into())
                .unwrap(),
            23.
        );
        assert_eq!(product.get_element_value(&(1, 0).into()).unwrap(), None);
        assert_eq!(product.get_element_value(&(0, 1).into()).unwrap(), None);
        assert_eq!(
            product
                .get_element_value_or_default(&(1, 1).into())
                .unwrap(),
            46.
        );
    }
}
