#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OrthogonalNormalFormSchemaV1 {
    pub(super) profile_count: usize,
    pub(super) has_half_turn: bool,
    pub(super) has_reflection: bool,
    pub(super) has_twisted_reflection: bool,
}

impl OrthogonalNormalFormSchemaV1 {
    pub(super) fn state_width(self) -> Option<usize> {
        if self.has_twisted_reflection && (!self.has_half_turn || !self.has_reflection) {
            return None;
        }
        self.profile_count
            .checked_add(usize::from(self.has_half_turn))?
            .checked_add(usize::from(self.has_reflection))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DirectedOrthogonalLabelV1 {
    Primary { profile: usize, sign: i8 },
    PrimaryHalfTurn,
    Reflection,
    TwistedReflection,
}

impl DirectedOrthogonalLabelV1 {
    pub(super) fn inverse(self) -> Option<Self> {
        match self {
            Self::Primary { profile, sign } => Some(Self::Primary {
                profile,
                sign: sign.checked_neg()?,
            }),
            Self::PrimaryHalfTurn | Self::Reflection | Self::TwistedReflection => Some(self),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OrthogonalNormalFormV1 {
    free: Vec<i32>,
    primary_half_turn: Option<bool>,
    reflection: Option<bool>,
}

impl OrthogonalNormalFormV1 {
    pub(super) fn identity(schema: OrthogonalNormalFormSchemaV1) -> Option<Self> {
        schema.state_width()?;
        let mut free = Vec::new();
        free.try_reserve_exact(schema.profile_count).ok()?;
        free.resize(schema.profile_count, 0);
        Some(Self {
            free,
            primary_half_turn: schema.has_half_turn.then_some(false),
            reflection: schema.has_reflection.then_some(false),
        })
    }

    pub(super) fn right_product(
        &self,
        schema: OrthogonalNormalFormSchemaV1,
        label: DirectedOrthogonalLabelV1,
    ) -> Option<Self> {
        if self.free.len() != schema.profile_count
            || self.primary_half_turn.is_some() != schema.has_half_turn
            || self.reflection.is_some() != schema.has_reflection
            || schema.state_width()? == 0
        {
            return None;
        }
        let mut free = Vec::new();
        free.try_reserve_exact(schema.profile_count).ok()?;
        free.extend_from_slice(&self.free);
        let mut result = Self {
            free,
            primary_half_turn: self.primary_half_turn,
            reflection: self.reflection,
        };
        match label {
            DirectedOrthogonalLabelV1::Primary { profile, sign } => {
                if profile >= schema.profile_count || !matches!(sign, -1 | 1) {
                    return None;
                }
                let delta = if self.reflection.unwrap_or(false) {
                    i32::from(sign).checked_neg()?
                } else {
                    i32::from(sign)
                };
                result.free[profile] = result.free[profile].checked_add(delta)?;
            }
            DirectedOrthogonalLabelV1::PrimaryHalfTurn => {
                let value = result.primary_half_turn.as_mut()?;
                *value = !*value;
            }
            DirectedOrthogonalLabelV1::Reflection => {
                let value = result.reflection.as_mut()?;
                *value = !*value;
            }
            DirectedOrthogonalLabelV1::TwistedReflection => {
                if !schema.has_twisted_reflection {
                    return None;
                }
                let half_turn = result.primary_half_turn.as_mut()?;
                *half_turn = !*half_turn;
                let reflection = result.reflection.as_mut()?;
                *reflection = !*reflection;
            }
        }
        Some(result)
    }

    #[cfg(test)]
    pub(super) fn components(&self) -> (&[i32], Option<bool>, Option<bool>) {
        (&self.free, self.primary_half_turn, self.reflection)
    }
}
