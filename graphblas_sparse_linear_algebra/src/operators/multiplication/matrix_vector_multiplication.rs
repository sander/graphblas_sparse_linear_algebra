use std::marker::PhantomData;
use std::ptr;

use crate::collections::sparse_matrix::{GraphblasSparseMatrixTrait, SparseMatrix};
use crate::collections::sparse_vector::{GraphblasSparseVectorTrait, SparseVector};
use crate::context::{CallGraphBlasContext, ContextTrait};
use crate::error::SparseLinearAlgebraError;
use crate::operators::binary_operator::{AccumulatorBinaryOperator, BinaryOperator};
use crate::operators::options::OperatorOptions;
use crate::operators::semiring::Semiring;
use crate::value_type::{AsBoolean, ValueType};

use crate::bindings_to_graphblas_implementation::{
    GrB_BinaryOp, GrB_Descriptor, GrB_Semiring, GrB_mxv,
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
    for MatrixVectorMultiplicationOperator<FirstArgument, SecondArgument, Product, EvaluationDomain>
{
}
unsafe impl<
        FirstArgument: ValueType,
        SecondArgument: ValueType,
        Product: ValueType,
        EvaluationDomain: ValueType,
    > Sync
    for MatrixVectorMultiplicationOperator<FirstArgument, SecondArgument, Product, EvaluationDomain>
{
}

#[derive(Debug, Clone)]
pub struct MatrixVectorMultiplicationOperator<Multiplier, Multiplicant, Product, EvaluationDomain>
where
    Multiplier: ValueType,
    Multiplicant: ValueType,
    Product: ValueType,
    EvaluationDomain: ValueType,
{
    _multiplier: PhantomData<Multiplier>,
    _multiplicant: PhantomData<Multiplicant>,
    _product: PhantomData<Product>,
    _evaluation_domain: PhantomData<EvaluationDomain>,

    accumulator: GrB_BinaryOp, // determines how results are written into the result matrix C
    semiring: GrB_Semiring,    // defines '+' and '*' for A*B (not optional for GrB_mxm)
    options: GrB_Descriptor,
}

impl<Multiplier, Multiplicant, Product, EvaluationDomain>
    MatrixVectorMultiplicationOperator<Multiplier, Multiplicant, Product, EvaluationDomain>
where
    Multiplier: ValueType,
    Multiplicant: ValueType,
    Product: ValueType,
    EvaluationDomain: ValueType,
{
    pub fn new(
        semiring: &impl Semiring<Multiplier, Multiplicant, Product, EvaluationDomain>, // defines '+' and '*' for A*B (not optional for GrB_mxm)
        options: &OperatorOptions,
        accumulator: &impl AccumulatorBinaryOperator<Product, Product, Product, EvaluationDomain>, // determines how results are written into the result matrix C
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

pub trait MultiplyMatrixByVector<Multiplier: ValueType, Multiplicant: ValueType, Product: ValueType>
{
    // TODO: consider a version where the resulting product matrix is generated in the function body
    fn apply(
        &self,
        // mask: Option<&SparseMatrix<AsBoolean<ValueType>>>,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseVector<Multiplicant>,
        product: &mut SparseVector<Product>,
    ) -> Result<(), SparseLinearAlgebraError>;

    fn apply_with_mask<MaskValueType: ValueType + AsBoolean>(
        &self,
        mask: &SparseVector<MaskValueType>,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseVector<Multiplicant>,
        product: &mut SparseVector<Product>,
    ) -> Result<(), SparseLinearAlgebraError>;
}

impl<
        Multiplier: ValueType,
        Multiplicant: ValueType,
        Product: ValueType,
        EvaluationDomain: ValueType,
    > MultiplyMatrixByVector<Multiplier, Multiplicant, Product>
    for MatrixVectorMultiplicationOperator<Multiplier, Multiplicant, Product, EvaluationDomain>
{
    // TODO: consider a version where the resulting product matrix is generated in the function body
    fn apply(
        &self,
        // mask: Option<&SparseMatrix<AsBoolean<ValueType>>>,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseVector<Multiplicant>,
        product: &mut SparseVector<Product>,
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = product.context();

        context.call(
            || unsafe {
                GrB_mxv(
                    product.graphblas_vector(),
                    ptr::null_mut(),
                    self.accumulator,
                    self.semiring,
                    multiplier.graphblas_matrix(),
                    multiplicant.graphblas_vector(),
                    self.options,
                )
            },
            unsafe { product.graphblas_vector_ref() },
        )?;

        Ok(())
    }

    fn apply_with_mask<MaskValueType: ValueType + AsBoolean>(
        &self,
        mask: &SparseVector<MaskValueType>,
        multiplier: &SparseMatrix<Multiplier>,
        multiplicant: &SparseVector<Multiplicant>,
        product: &mut SparseVector<Product>,
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = product.context();

        context.call(
            || unsafe {
                GrB_mxv(
                    product.graphblas_vector(),
                    mask.graphblas_vector(),
                    self.accumulator,
                    self.semiring,
                    multiplier.graphblas_matrix(),
                    multiplicant.graphblas_vector(),
                    self.options,
                )
            },
            unsafe { product.graphblas_vector_ref() },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::collections::sparse_matrix::{FromMatrixElementList, MatrixElementList, Size};
    use crate::collections::sparse_vector::{
        FromVectorElementList, GetVectorElementList, GetVectorElementValue, VectorElementList,
    };
    use crate::collections::Collection;
    use crate::context::{Context, Mode};
    use crate::operators::binary_operator::Plus;
    use crate::operators::binary_operator::{Assignment, First};
    use crate::operators::semiring::PlusTimes;

    #[test]
    fn test_multiplication_with_plus_times() {
        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let semiring = PlusTimes::<f32, f32, f32, f32>::new();
        let options = OperatorOptions::new_default();
        let matrix_multiplier = MatrixVectorMultiplicationOperator::<f32, f32, f32, f32>::new(
            &semiring,
            &options,
            &Assignment::new(),
        );

        let length = 2;
        let size: Size = (length, length).into();

        let multiplier = SparseMatrix::<f32>::new(&context, &size).unwrap();
        let multiplicant = SparseVector::<f32>::new(&context, &length).unwrap();
        let mut product = multiplicant.clone();

        // Test multiplication of empty matrices
        matrix_multiplier
            .apply(&multiplier, &multiplicant, &mut product)
            .unwrap();
        let element_list = product.get_element_list().unwrap();

        assert_eq!(product.number_of_stored_elements().unwrap(), 0);
        assert_eq!(element_list.length(), 0);
        assert_eq!(product.get_element_value(&1).unwrap(), None); // NoValue

        let multiplicant_element_list =
            VectorElementList::<f32>::from_element_vector(vec![(0, 1.0).into(), (1, 2.0).into()]);
        let multiplicant = SparseVector::<f32>::from_element_list(
            &context,
            &length,
            &multiplicant_element_list,
            &First::<f32, f32, f32, f32>::new(),
        )
        .unwrap();

        let multiplier_element_list = MatrixElementList::<f32>::from_element_vector(vec![
            (0, 0, 5.0).into(),
            (1, 0, 6.0).into(),
            (0, 1, 7.0).into(),
            (1, 1, 8.0).into(),
        ]);
        let multiplier = SparseMatrix::<f32>::from_element_list(
            &context,
            &size,
            &multiplier_element_list,
            &First::<f32, f32, f32, f32>::new(),
        )
        .unwrap();

        // Test multiplication of full matrices
        matrix_multiplier
            .apply(&multiplier, &multiplicant, &mut product)
            .unwrap();

        assert_eq!(product.get_element_value_or_default(&0).unwrap(), 19.);
        assert_eq!(product.get_element_value_or_default(&1).unwrap(), 22.);

        // TODO: this test is not generic over column/row storage format.
        // Equality checks should be done at a matrix level, since the ordering of the element list is not guaranteed.
        let expected_product =
            VectorElementList::<f32>::from_element_vector(vec![(0, 19.).into(), (1, 22.).into()]);
        let product_element_list = product.get_element_list().unwrap();
        assert_eq!(expected_product, product_element_list);

        // test the use of an accumulator
        let accumulator = Plus::<f32, f32, f32, f32>::new();
        let matrix_multiplier_with_accumulator = MatrixVectorMultiplicationOperator::<
            f32,
            f32,
            f32,
            f32,
        >::new(&semiring, &options, &accumulator);

        matrix_multiplier_with_accumulator
            .apply(&multiplier, &multiplicant, &mut product)
            .unwrap();

        assert_eq!(product.get_element_value_or_default(&0).unwrap(), 19. * 2.);
        assert_eq!(product.get_element_value_or_default(&1).unwrap(), 22. * 2.);

        // test the use of a mask
        let mask_element_list =
            VectorElementList::<u8>::from_element_vector(vec![(0, 3).into(), (1, 0).into()]);
        let mask = SparseVector::<u8>::from_element_list(
            &context,
            &length,
            &mask_element_list,
            &First::<u8, u8, u8, u8>::new(),
        )
        .unwrap();

        let matrix_multiplier = MatrixVectorMultiplicationOperator::<f32, f32, f32, f32>::new(
            &semiring,
            &options,
            &Assignment::new(),
        );

        let mut product = SparseVector::<f32>::new(&context, &length).unwrap();

        matrix_multiplier
            .apply_with_mask(&mask, &multiplier, &multiplicant, &mut product)
            .unwrap();

        assert_eq!(product.get_element_value_or_default(&0).unwrap(), 19.);
        assert_eq!(product.get_element_value(&1).unwrap(), None);
    }
}
