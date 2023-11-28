use crate::collections::sparse_matrix::operations::GetSparseMatrixSize;
use crate::collections::sparse_matrix::GetGraphblasSparseMatrix;
use crate::collections::sparse_vector::{GetGraphblasSparseVector, SparseVector};
use crate::context::{CallGraphBlasContext, GetContext};
use crate::error::SparseLinearAlgebraError;
use crate::index::{
    ElementIndex, ElementIndexSelector, ElementIndexSelectorGraphblasType, IndexConversion,
};
use crate::operators::binary_operator::AccumulatorBinaryOperator;
use crate::operators::mask::VectorMask;
use crate::operators::options::{OperatorOptions, OperatorOptionsTrait};
use crate::value_type::ValueType;

use crate::graphblas_bindings::GrB_Col_extract;

// Implemented methods do not provide mutable access to GraphBLAS operators or options.
// Code review must consider that no mtable access is provided.
// https://doc.rust-lang.org/nomicon/send-and-sync.html
unsafe impl Sync for MatrixColumnExtractor {}
unsafe impl Send for MatrixColumnExtractor {}

#[derive(Debug, Clone)]
pub struct MatrixColumnExtractor {}

impl MatrixColumnExtractor {
    pub fn new() -> Self {
        Self {}
    }
}

pub trait ExtractMatrixColumn<Column: ValueType> {
    fn apply(
        &self,
        matrix_to_extract_from: &(impl GetGraphblasSparseMatrix + GetSparseMatrixSize + GetContext),
        column_index_to_extract: &ElementIndex,
        indices_to_extract: &ElementIndexSelector,
        accumulator: &impl AccumulatorBinaryOperator<Column>,
        column_vector: &mut (impl GetGraphblasSparseVector + GetContext),
        mask: &(impl VectorMask + GetContext),
        options: &OperatorOptions,
    ) -> Result<(), SparseLinearAlgebraError>;
}

impl<Column: ValueType> ExtractMatrixColumn<Column> for MatrixColumnExtractor {
    fn apply(
        &self,
        matrix_to_extract_from: &(impl GetGraphblasSparseMatrix + GetSparseMatrixSize + GetContext),
        column_index_to_extract: &ElementIndex,
        indices_to_extract: &ElementIndexSelector,
        accumulator: &impl AccumulatorBinaryOperator<Column>,
        column_vector: &mut (impl GetGraphblasSparseVector + GetContext),
        mask: &(impl VectorMask + GetContext),
        options: &OperatorOptions,
    ) -> Result<(), SparseLinearAlgebraError> {
        let context = matrix_to_extract_from.context();

        let number_of_indices_to_extract: ElementIndex;
        match indices_to_extract {
            ElementIndexSelector::Index(indices) => number_of_indices_to_extract = indices.len(),
            ElementIndexSelector::All => {
                number_of_indices_to_extract = matrix_to_extract_from.row_height()?
            }
        }
        let number_of_indices_to_extract = number_of_indices_to_extract.to_graphblas_index()?;

        let indices_to_extract = indices_to_extract.to_graphblas_type()?;

        let column_index_to_extract = column_index_to_extract.to_graphblas_index()?;

        match indices_to_extract {
            ElementIndexSelectorGraphblasType::Index(index) => {
                context.call(
                    || unsafe {
                        GrB_Col_extract(
                            GetGraphblasSparseVector::graphblas_vector(column_vector),
                            mask.graphblas_vector(),
                            accumulator.accumulator_graphblas_type(),
                            matrix_to_extract_from.graphblas_matrix(),
                            index.as_ptr(),
                            number_of_indices_to_extract,
                            column_index_to_extract,
                            options.to_graphblas_descriptor(),
                        )
                    },
                    unsafe { column_vector.graphblas_vector_ref() },
                )?;
            }
            ElementIndexSelectorGraphblasType::All(index) => {
                context.call(
                    || unsafe {
                        GrB_Col_extract(
                            GetGraphblasSparseVector::graphblas_vector(column_vector),
                            mask.graphblas_vector(),
                            accumulator.accumulator_graphblas_type(),
                            matrix_to_extract_from.graphblas_matrix(),
                            index,
                            number_of_indices_to_extract,
                            column_index_to_extract,
                            options.to_graphblas_descriptor(),
                        )
                    },
                    unsafe { column_vector.graphblas_vector_ref() },
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::collections::sparse_matrix::operations::FromMatrixElementList;
    use crate::collections::sparse_matrix::{MatrixElementList, SparseMatrix};
    use crate::collections::sparse_vector::operations::GetVectorElementValue;
    use crate::collections::Collection;
    use crate::context::{Context, Mode};
    use crate::operators::binary_operator::{Assignment, First};
    use crate::operators::mask::SelectEntireVector;

    #[test]
    fn test_column_extraction() {
        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let element_list = MatrixElementList::<u8>::from_element_vector(vec![
            (0, 0, 1).into(),
            (1, 0, 2).into(),
            (2, 0, 3).into(),
            (0, 1, 4).into(),
            (1, 1, 5).into(),
            (2, 1, 6).into(),
        ]);

        let matrix = SparseMatrix::<u8>::from_element_list(
            &context.to_owned(),
            &(3, 2).into(),
            &element_list,
            &First::<u8>::new(),
        )
        .unwrap();

        let mut column_vector = SparseVector::<u8>::new(&context, &2).unwrap();

        let indices_to_extract: Vec<ElementIndex> = vec![0, 2];
        let indices_to_extract = ElementIndexSelector::Index(&indices_to_extract);

        let extractor = MatrixColumnExtractor::new();

        extractor
            .apply(
                &matrix,
                &0,
                &indices_to_extract,
                &Assignment::<u8>::new(),
                &mut column_vector,
                &SelectEntireVector::new(&context),
                &OperatorOptions::new_default(),
            )
            .unwrap();

        assert_eq!(column_vector.number_of_stored_elements().unwrap(), 2);
        assert_eq!(column_vector.element_value_or_default(&0).unwrap(), 1);
        // assert_eq!(column_vector.get_element_value(&1).unwrap(), 0);
        assert_eq!(column_vector.element_value_or_default(&1).unwrap(), 3);
    }

    #[test]
    fn test_column_extraction_with_type_casting() {
        let context = Context::init_ready(Mode::NonBlocking).unwrap();

        let element_list = MatrixElementList::<u16>::from_element_vector(vec![
            (0, 0, 1).into(),
            (1, 0, 2).into(),
            (2, 0, 3).into(),
            (0, 1, 4).into(),
            (1, 1, 5).into(),
            (2, 1, 6).into(),
        ]);

        let matrix = SparseMatrix::<u16>::from_element_list(
            &context.to_owned(),
            &(3, 2).into(),
            &element_list,
            &First::<u16>::new(),
        )
        .unwrap();

        let mut column_vector = SparseVector::<u8>::new(&context, &2).unwrap();

        let indices_to_extract: Vec<ElementIndex> = vec![0, 2];
        let indices_to_extract = ElementIndexSelector::Index(&indices_to_extract);

        let extractor = MatrixColumnExtractor::new();

        extractor
            .apply(
                &matrix,
                &0,
                &indices_to_extract,
                &Assignment::<f32>::new(),
                &mut column_vector,
                &SelectEntireVector::new(&context),
                &OperatorOptions::new_default(),
            )
            .unwrap();

        assert_eq!(column_vector.number_of_stored_elements().unwrap(), 2);
        assert_eq!(column_vector.element_value_or_default(&0).unwrap(), 1);
        // assert_eq!(column_vector.get_element_value(&1).unwrap(), 0);
        assert_eq!(column_vector.element_value_or_default(&1).unwrap(), 3);
    }
}
