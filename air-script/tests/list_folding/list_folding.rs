use winter_air::{Air, AirContext, Assertion, AuxTraceRandElements, EvaluationFrame, ProofOptions as WinterProofOptions, TransitionConstraintDegree, TraceInfo};
use winter_math::fields::f64::BaseElement as Felt;
use winter_math::{ExtensionOf, FieldElement};
use winter_utils::collections::Vec;
use winter_utils::{ByteWriter, Serializable};

pub struct PublicInputs {
    stack_inputs: [Felt; 16],
}

impl PublicInputs {
    pub fn new(stack_inputs: [Felt; 16]) -> Self {
        Self { stack_inputs }
    }
}

impl Serializable for PublicInputs {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        target.write(self.stack_inputs.as_slice());
    }
}

pub struct ListFoldingAir {
    context: AirContext<Felt>,
    stack_inputs: [Felt; 16],
}

impl ListFoldingAir {
    pub fn last_step(&self) -> usize {
        self.trace_length() - self.context().num_transition_exemptions()
    }
}

impl Air for ListFoldingAir {
    type BaseField = Felt;
    type PublicInputs = PublicInputs;

    fn context(&self) -> &AirContext<Felt> {
        &self.context
    }

    fn new(trace_info: TraceInfo, public_inputs: PublicInputs, options: WinterProofOptions) -> Self {
        let main_degrees = vec![];
        let aux_degrees = vec![TransitionConstraintDegree::new(4), TransitionConstraintDegree::new(4), TransitionConstraintDegree::new(4), TransitionConstraintDegree::new(2)];
        let num_main_assertions = 0;
        let num_aux_assertions = 1;

        let context = AirContext::new_multi_segment(
            trace_info,
            main_degrees,
            aux_degrees,
            num_main_assertions,
            num_aux_assertions,
            options,
        )
        .set_num_transition_exemptions(2);
        Self { context, stack_inputs: public_inputs.stack_inputs }
    }

    fn get_periodic_column_values(&self) -> Vec<Vec<Felt>> {
        vec![]
    }

    fn get_assertions(&self) -> Vec<Assertion<Felt>> {
        let mut result = Vec::new();
        result
    }

    fn get_aux_assertions<E: FieldElement<BaseField = Felt>>(&self, aux_rand_elements: &AuxTraceRandElements<E>) -> Vec<Assertion<E>> {
        let mut result = Vec::new();
        result.push(Assertion::single(7, 0, E::ZERO));
        result
    }

    fn evaluate_transition<E: FieldElement<BaseField = Felt>>(&self, frame: &EvaluationFrame<E>, periodic_values: &[E], result: &mut [E]) {
        let main_current = frame.current();
        let main_next = frame.next();
    }

    fn evaluate_aux_transition<F, E>(&self, main_frame: &EvaluationFrame<F>, aux_frame: &EvaluationFrame<E>, _periodic_values: &[F], aux_rand_elements: &AuxTraceRandElements<E>, result: &mut [E])
    where F: FieldElement<BaseField = Felt>,
          E: FieldElement<BaseField = Felt> + ExtensionOf<F>,
    {
        let main_current = main_frame.current();
        let main_next = main_frame.next();
        let aux_current = aux_frame.current();
        let aux_next = aux_frame.next();
        result[0] = aux_next[1] - (aux_current[5] + aux_current[6] + aux_current[7] + aux_current[8] + aux_current[9] * aux_current[10] * aux_current[11] * aux_current[12]);
        result[1] = aux_next[2] - (aux_current[5] + aux_current[6] + aux_current[7] + aux_current[8] + aux_current[9] * aux_current[10] * aux_current[11] * aux_current[12]);
        result[2] = aux_next[3] - (aux_current[5] * aux_current[9] + aux_current[6] * aux_current[10] + aux_current[7] * aux_current[11] + aux_current[8] * aux_current[12] + (aux_current[5] + aux_current[9]) * (aux_current[6] + aux_current[10]) * (aux_current[7] + aux_current[11]) * (aux_current[8] + aux_current[12]));
        result[3] = aux_next[4] - (E::from(main_current[1]) + aux_current[5] * aux_current[9] + aux_current[6] * aux_current[10] + aux_current[7] * aux_current[11] + aux_current[8] * aux_current[12] + aux_current[5] * aux_current[9] + aux_current[6] * aux_current[10] + aux_current[7] * aux_current[11] + aux_current[8] * aux_current[12]);
    }
}