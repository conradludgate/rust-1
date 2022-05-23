use crate::infer::lexical_region_resolve::RegionResolutionError;
use crate::infer::lexical_region_resolve::RegionResolutionError::*;
use crate::infer::InferCtxt;
use rustc_errors::{DiagnosticBuilder, ErrorGuaranteed};
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::source_map::Span;

mod different_lifetimes;
pub mod find_anon_type;
mod mismatched_static_lifetime;
mod named_anon_conflict;
mod placeholder_error;
mod static_impl_trait;
mod trait_impl_difference;
mod util;

pub use different_lifetimes::suggest_adding_lifetime_params;
pub use find_anon_type::find_anon_type;
pub use static_impl_trait::suggest_new_region_bound;
pub use util::find_param_with_region;

impl<'cx, 'tcx> InferCtxt<'cx, 'tcx> {
    pub fn try_report_nice_region_error(&self, error: &RegionResolutionError<'tcx>) -> bool {
        NiceRegionError::new(self, error.clone()).try_report().is_some()
    }
}

pub struct NiceRegionError<'cx, 'tcx> {
    infcx: &'cx InferCtxt<'cx, 'tcx>,
    error: Option<RegionResolutionError<'tcx>>,
    regions: Option<(Span, ty::Region<'tcx>, ty::Region<'tcx>)>,
}

impl<'cx, 'tcx> NiceRegionError<'cx, 'tcx> {
    pub fn new(infcx: &'cx InferCtxt<'cx, 'tcx>, error: RegionResolutionError<'tcx>) -> Self {
        Self { infcx, error: Some(error), regions: None }
    }

    pub fn new_from_span(
        infcx: &'cx InferCtxt<'cx, 'tcx>,
        span: Span,
        sub: ty::Region<'tcx>,
        sup: ty::Region<'tcx>,
    ) -> Self {
        Self { infcx, error: None, regions: Some((span, sub, sup)) }
    }

    fn tcx(&self) -> TyCtxt<'tcx> {
        self.infcx.tcx
    }

    pub fn try_report_from_nll(&self) -> Option<DiagnosticBuilder<'tcx, ErrorGuaranteed>> {
        // Due to the improved diagnostics returned by the MIR borrow checker, only a subset of
        // the nice region errors are required when running under the MIR borrow checker.
        self.try_report_named_anon_conflict().or_else(|| self.try_report_placeholder_conflict())
    }

    pub fn try_report(&self) -> Option<ErrorGuaranteed> {
        self.try_report_from_nll()
            .map(|mut diag| diag.emit())
            .or_else(|| self.try_report_impl_not_conforming_to_trait())
            .or_else(|| self.try_report_anon_anon_conflict())
            .or_else(|| self.try_report_static_impl_trait())
            .or_else(|| self.try_report_mismatched_static_lifetime())
    }

    pub(super) fn regions(&self) -> Option<(Span, ty::Region<'tcx>, ty::Region<'tcx>)> {
        match (&self.error, self.regions) {
            (Some(ConcreteFailure(origin, sub, sup)), None) => Some((origin.span(), *sub, *sup)),
            (Some(SubSupConflict(_, _, origin, sub, _, sup, _)), None) => {
                Some((origin.span(), *sub, *sup))
            }
            (None, Some((span, sub, sup))) => Some((span, sub, sup)),
            _ => None,
        }
    }
}
