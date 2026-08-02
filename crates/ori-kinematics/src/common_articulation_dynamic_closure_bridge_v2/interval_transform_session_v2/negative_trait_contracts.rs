use super::*;

macro_rules! assert_not_impl {
    ($type:ty, $trait:path) => {
        const _: fn() = || {
            struct Invalid;
            trait AmbiguousIfImpl<A> {
                fn marker() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}
            let _ = <$type as AmbiguousIfImpl<_>>::marker;
        };
    };
}

assert_not_impl!(
    CommonArticulationDynamicClosureIntervalTransformSessionV2<'static>,
    Clone
);
assert_not_impl!(
    CommonArticulationDynamicClosureIntervalTransformSessionV2<'static>,
    serde::Serialize
);
assert_not_impl!(
    CommonArticulationDynamicClosureIntervalTransformSessionV2<'static>,
    std::ops::Deref
);
assert_not_impl!(
    CommonArticulationDynamicClosureIntervalTransformLeafV2<'static>,
    Clone
);
assert_not_impl!(
    CommonArticulationDynamicClosureIntervalTransformLeafV2<'static>,
    serde::Serialize
);
assert_not_impl!(
    CommonArticulationDynamicClosureIntervalTransformLeafV2<'static>,
    std::ops::Deref
);
assert_not_impl!(IntervalFaceTransformWorkspaceBoundV2, Clone);
assert_not_impl!(IntervalFaceTransformWorkspaceBoundV2, serde::Serialize);
assert_not_impl!(IntervalFaceTransformWorkspaceBoundV2, std::ops::Deref);
assert_not_impl!(WorkspaceBoundedMaterialFaceTransformRegistryV2, Clone);
assert_not_impl!(
    WorkspaceBoundedMaterialFaceTransformRegistryV2,
    serde::Serialize
);
assert_not_impl!(
    WorkspaceBoundedMaterialFaceTransformRegistryV2,
    std::ops::Deref
);
