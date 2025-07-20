use bevy::prelude::*;

#[derive(Component)]
pub struct Unit {
    /// How far above the sampled height the unit’s “feet” sit.
    pub grounded_offset: f32,
    /// Max angle (in radians) the unit can climb. 0 = flat only.
    pub max_slope: f32,
}

#[derive(Component)]
pub struct MoveTo(pub Vec3);

/// Marks “ground-snappable” entities.
/// `offset` = distance from ground sample to origin.
#[derive(Component)]
pub struct Grounded {
    pub offset: f32,
}

#[derive(Component, Deref, DerefMut)]
pub struct PreviousPosition(pub Vec3);