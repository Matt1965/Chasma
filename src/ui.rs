use bevy::prelude::*;
use bevy::ui::BackgroundColor;

#[derive(Component)]
pub struct PauseOverlay;

pub fn spawn_pause_overlay(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        // Fullscreen transparent overlay node
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.7)),
        PauseOverlay,
    ))
    .with_children(|parent| {
        parent.spawn((
            // The UI text component
            Text::new("Paused"),
            TextFont {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 64.0,
                ..default()
            },
            TextLayout::new_with_justify(JustifyText::Center),
            TextColor(Color::WHITE),
        ));
    });
}

pub fn despawn_pause_overlay(
    mut commands: Commands,
    query: Query<Entity, With<PauseOverlay>>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}



