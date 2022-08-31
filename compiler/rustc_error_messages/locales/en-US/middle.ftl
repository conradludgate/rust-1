middle_drop_check_overflow =
    overflow while adding drop-check rules for {$ty}
    .note = {$note}

middle_opaque_hidden_type_mismatch =
    concrete type differs from previous defining opaque type use
    .label = expected `{$self_ty}`, got `{$other_ty}`

middle_conflict_types =
    this expression supplies two conflicting concrete types for the same opaque type

middle_previous_use_here =
    previous use here
