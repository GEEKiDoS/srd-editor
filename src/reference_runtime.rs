use std::fmt;

use crate::animation::RuntimeAnimationState;
use crate::csli::{add_color_saturating_game, multiply_color_game};
use crate::image::{ImageDefinition, RuntimeImageState};
use crate::reference::ReferenceAnimationRequest;
use crate::scene::{AnimationSetDefinition, Layer, Project, ReferenceTarget};
use crate::texture::TextureList;
use crate::transform::{Affine3x4, SpatialTransform, build_local_matrix};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceLayerParent {
    ProjectLayer(ReferenceTarget),
    ReferenceInstance(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceLayerInstance {
    pub parent: ReferenceLayerParent,
    pub reference_node_index: usize,
    pub target: ReferenceTarget,
    pub is_2d: bool,
    pub flip_y: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeWorldState {
    pub matrix: Affine3x4,
    pub multiply_color: [u8; 4],
    pub additive_color: [u8; 4],
    pub visible: bool,
    pub render_gate: bool,
}

impl Default for RuntimeWorldState {
    fn default() -> Self {
        Self {
            matrix: Affine3x4::IDENTITY,
            multiply_color: [255; 4],
            additive_color: [0; 4],
            visible: true,
            render_gate: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceLayerLocalState {
    pub transform: SpatialTransform,
    pub enabled: bool,
}

impl Default for ReferenceLayerLocalState {
    fn default() -> Self {
        Self {
            transform: SpatialTransform::default(),
            enabled: true,
        }
    }
}

impl ReferenceLayerInstance {
    pub fn compose_world_state(
        &self,
        parent_cast: RuntimeWorldState,
        local: ReferenceLayerLocalState,
    ) -> RuntimeWorldState {
        let local_matrix =
            build_local_matrix(&local.transform, self.is_2d, self.flip_y, [0.0, 0.0]);
        RuntimeWorldState {
            matrix: parent_cast.matrix.mul_game(local_matrix),
            multiply_color: multiply_color_game(
                parent_cast.multiply_color,
                local.transform.multiply_color,
            ),
            additive_color: add_color_saturating_game(
                parent_cast.additive_color,
                local.transform.additive_color,
            ),
            visible: parent_cast.visible && local.enabled,
            render_gate: parent_cast.render_gate && local.enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedReference {
    pub parent: ReferenceLayerParent,
    pub reference_node_index: usize,
    pub source_name: Vec<u8>,
    pub layer_name: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReferenceRuntimePlan {
    pub instances: Vec<ReferenceLayerInstance>,
    pub unresolved: Vec<UnresolvedReference>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceLayerRuntimeState {
    pub instance_index: usize,
    pub local: ReferenceLayerLocalState,
    pub cast_transforms: Vec<SpatialTransform>,
    pub image_bases: Vec<ImageDefinition>,
    pub image_states: Vec<RuntimeImageState>,
    pub animations: Vec<RuntimeAnimationState>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeAnimationApplication {
    pub common_channels: usize,
    pub image_channels: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeAnimationTreeApplication {
    pub animated_layers: usize,
    pub common_channels: usize,
    pub image_channels: usize,
    pub reference_requests: usize,
}

impl RuntimeAnimationTreeApplication {
    fn include_layer(&mut self, application: RuntimeAnimationApplication) {
        self.animated_layers += 1;
        self.common_channels += application.common_channels;
        self.image_channels += application.image_channels;
    }

    fn include_tree(&mut self, child: Self) {
        self.animated_layers += child.animated_layers;
        self.common_channels += child.common_channels;
        self.image_channels += child.image_channels;
        self.reference_requests += child.reference_requests;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceRuntime {
    pub plan: ReferenceRuntimePlan,
    pub layers: Vec<ReferenceLayerRuntimeState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectLayerRuntimeState {
    pub target: ReferenceTarget,
    pub enabled: bool,
    pub cast_transforms: Vec<SpatialTransform>,
    pub image_bases: Vec<ImageDefinition>,
    pub image_states: Vec<RuntimeImageState>,
    pub animations: Vec<RuntimeAnimationState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRuntime {
    pub project_layers: Vec<Vec<ProjectLayerRuntimeState>>,
    pub references: ReferenceRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceRuntimeError {
    pub repeating_layers: Vec<ReferenceTarget>,
}

impl fmt::Display for ReferenceRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("reference-layer construction does not converge through")?;
        for layer in &self.repeating_layers {
            write!(
                formatter,
                " SCN[{}]/LAYR[{}]",
                layer.scene_index, layer.layer_index
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ReferenceRuntimeError {}

impl Project {
    pub fn build_reference_runtime_plan(
        &self,
    ) -> Result<ReferenceRuntimePlan, ReferenceRuntimeError> {
        let mut plan = ReferenceRuntimePlan::default();
        let mut lineages = Vec::new();

        // The binary resolves all CASTs in the original runtime scene table before it
        // drains the queue of newly constructed reference layers.
        for (scene_index, scene) in self.scenes.iter().enumerate() {
            for (layer_index, layer) in scene.layers.iter().enumerate() {
                let source = ReferenceTarget {
                    scene_index,
                    layer_index,
                };
                append_layer_references(
                    self,
                    source,
                    ReferenceLayerParent::ProjectLayer(source),
                    layer.is_2d(),
                    &[source],
                    &mut plan,
                    &mut lineages,
                )?;
            }
        }

        // Each appended instance corresponds to one fresh runtime layer. Processing
        // the growing vector in order reproduces the binary's successive queue waves.
        let mut instance_index = 0usize;
        while instance_index < plan.instances.len() {
            let instance = plan.instances[instance_index].clone();
            let lineage = lineages[instance_index].clone();
            append_layer_references(
                self,
                instance.target,
                ReferenceLayerParent::ReferenceInstance(instance_index),
                instance.is_2d,
                &lineage,
                &mut plan,
                &mut lineages,
            )?;
            instance_index += 1;
        }

        Ok(plan)
    }
}

impl ReferenceLayerRuntimeState {
    pub fn new(
        project: &Project,
        plan: &ReferenceRuntimePlan,
        instance_index: usize,
    ) -> Option<Self> {
        let instance = plan.instances.get(instance_index)?;
        let layer =
            &project.scenes[instance.target.scene_index].layers[instance.target.layer_index];
        let image_bases = runtime_image_bases(layer);
        let image_states = runtime_image_states(layer, &image_bases);
        Some(Self {
            instance_index,
            local: ReferenceLayerLocalState::default(),
            cast_transforms: layer
                .transforms
                .iter()
                .copied()
                .map(|transform| transform.spatial())
                .collect(),
            image_bases,
            image_states,
            animations: layer
                .animations
                .iter()
                .map(|animation| animation.initial_runtime_state())
                .collect(),
        })
    }

    pub fn apply_animation_common_channels(
        &mut self,
        project: &Project,
        plan: &ReferenceRuntimePlan,
        animation_name: &[u8],
        frame: f32,
    ) -> Result<Option<usize>, crate::animation::AnimationError> {
        let instance = &plan.instances[self.instance_index];
        let layer =
            &project.scenes[instance.target.scene_index].layers[instance.target.layer_index];
        let Some((animation_index, animation)) = layer.find_animation(animation_name) else {
            return Ok(None);
        };
        self.animations[animation_index].frame = frame;
        animation
            .apply_common_channels(&mut self.cast_transforms, frame)
            .map(Some)
    }

    pub fn apply_reference_request_common_channels(
        &mut self,
        project: &Project,
        plan: &ReferenceRuntimePlan,
        request: ReferenceAnimationRequest<'_>,
    ) -> Result<Option<usize>, crate::animation::AnimationError> {
        self.apply_animation_common_channels(project, plan, request.animation_name, request.frame)
    }

    pub fn apply_animation_channels(
        &mut self,
        project: &Project,
        plan: &ReferenceRuntimePlan,
        textures: &TextureList,
        animation_name: &[u8],
        frame: f32,
    ) -> Result<Option<RuntimeAnimationApplication>, crate::animation::AnimationError> {
        let instance = &plan.instances[self.instance_index];
        let layer =
            &project.scenes[instance.target.scene_index].layers[instance.target.layer_index];
        apply_runtime_layer_channels(
            layer,
            textures,
            animation_name,
            frame,
            &mut self.cast_transforms,
            &self.image_bases,
            &mut self.image_states,
            &mut self.animations,
        )
    }
}

fn runtime_image_bases(layer: &Layer) -> Vec<ImageDefinition> {
    layer
        .nodes
        .iter()
        .enumerate()
        .map(|(node_index, node)| match node.cast_type() {
            Some(1) => layer.image_by_node[node_index]
                .clone()
                .unwrap_or_else(ImageDefinition::srimage_constructor_base),
            Some(2) => layer.csli_by_node[node_index]
                .as_ref()
                .map(ImageDefinition::from_csli_runtime_base)
                .unwrap_or_else(ImageDefinition::srimage_constructor_base),
            Some(4) => layer.number_by_node[node_index]
                .as_ref()
                .map(crate::number::NumberDefinition::image_base)
                .unwrap_or_else(ImageDefinition::srimage_constructor_base),
            _ => ImageDefinition::srimage_constructor_base(),
        })
        .collect()
}

fn runtime_image_states(layer: &Layer, image_bases: &[ImageDefinition]) -> Vec<RuntimeImageState> {
    image_bases
        .iter()
        .enumerate()
        .map(|(node_index, image)| {
            let mut state = image.initial_runtime_state();
            if let Some(ext_param) = layer.ext_param_for_node(node_index) {
                state.render_preset_override = ext_param.render_preset_override;
            }
            state
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn apply_runtime_layer_channels(
    layer: &Layer,
    textures: &TextureList,
    animation_name: &[u8],
    frame: f32,
    cast_transforms: &mut [SpatialTransform],
    image_bases: &[ImageDefinition],
    image_states: &mut [RuntimeImageState],
    animations: &mut [RuntimeAnimationState],
) -> Result<Option<RuntimeAnimationApplication>, crate::animation::AnimationError> {
    let Some((animation_index, animation)) = layer.find_animation(animation_name) else {
        return Ok(None);
    };
    animations[animation_index].frame = frame;

    let mut application = RuntimeAnimationApplication::default();
    let cast_count = cast_transforms.len();
    for motion in &animation.motions {
        if motion.target < 0 {
            continue;
        }
        let node_index = usize::try_from(motion.target).map_err(|_| {
            crate::animation::AnimationError("MOT target does not fit usize".into())
        })?;
        if node_index >= cast_count {
            return Err(crate::animation::AnimationError(format!(
                "MOT target {node_index} is outside {cast_count} runtime CASTs"
            )));
        }
        application.common_channels +=
            motion.apply_proven_common_channels(&mut cast_transforms[node_index], frame);
        let base = &image_bases[node_index];
        let state = &mut image_states[node_index];
        for track in &motion.tracks {
            if base
                .apply_runtime_track(state, track, frame, textures)
                .map_err(|error| crate::animation::AnimationError(error.to_string()))?
            {
                application.image_channels += 1;
            }
        }
    }
    Ok(Some(application))
}

fn child_animation_requests(
    layer: &Layer,
    animation_name: &[u8],
    frame: f32,
    plan: &ReferenceRuntimePlan,
    parent: ReferenceLayerParent,
) -> Vec<(usize, Vec<u8>, f32)> {
    let Some((_, animation)) = layer.find_animation(animation_name) else {
        return Vec::new();
    };
    let mut requests = Vec::new();
    for motion in &animation.motions {
        let Ok(node_index) = usize::try_from(motion.target) else {
            continue;
        };
        let Some(reference) = layer
            .reference_by_node
            .get(node_index)
            .and_then(|definition| definition.as_ref())
        else {
            continue;
        };
        let child_instance = plan.instances.iter().position(|instance| {
            instance.parent == parent && instance.reference_node_index == node_index
        });
        let Some(child_instance) = child_instance else {
            continue;
        };
        for track in &motion.tracks {
            let Some(request) = reference.animation_request(track, frame) else {
                continue;
            };
            requests.push((
                child_instance,
                request.animation_name.to_vec(),
                request.frame,
            ));
        }
    }
    requests
}

impl ReferenceRuntime {
    pub fn new(project: &Project) -> Result<Self, ReferenceRuntimeError> {
        let plan = project.build_reference_runtime_plan()?;
        let layers = (0..plan.instances.len())
            .map(|instance_index| {
                ReferenceLayerRuntimeState::new(project, &plan, instance_index)
                    .expect("instance index came from the same reference runtime plan")
            })
            .collect();
        Ok(Self { plan, layers })
    }

    pub fn apply_instance_animation(
        &mut self,
        project: &Project,
        textures: &TextureList,
        instance_index: usize,
        animation_name: &[u8],
        frame: f32,
    ) -> Result<Option<RuntimeAnimationTreeApplication>, crate::animation::AnimationError> {
        if instance_index >= self.layers.len() {
            return Err(crate::animation::AnimationError(format!(
                "reference instance {instance_index} is outside {} runtime layers",
                self.layers.len()
            )));
        }

        let Some(application) = self.layers[instance_index].apply_animation_channels(
            project,
            &self.plan,
            textures,
            animation_name,
            frame,
        )?
        else {
            return Ok(None);
        };
        let mut tree = RuntimeAnimationTreeApplication::default();
        tree.include_layer(application);

        let target = self.plan.instances[instance_index].target;
        let layer = &project.scenes[target.scene_index].layers[target.layer_index];
        let child_requests = child_animation_requests(
            layer,
            animation_name,
            frame,
            &self.plan,
            ReferenceLayerParent::ReferenceInstance(instance_index),
        );

        for (child_instance, child_animation_name, child_frame) in child_requests {
            tree.reference_requests += 1;
            if let Some(child) = self.apply_instance_animation(
                project,
                textures,
                child_instance,
                &child_animation_name,
                child_frame,
            )? {
                tree.include_tree(child);
            }
        }
        Ok(Some(tree))
    }
}

impl ProjectLayerRuntimeState {
    fn new(target: ReferenceTarget, layer: &Layer) -> Self {
        let image_bases = runtime_image_bases(layer);
        let image_states = runtime_image_states(layer, &image_bases);
        Self {
            target,
            enabled: layer.flags & 0x100 != 0,
            cast_transforms: layer
                .transforms
                .iter()
                .copied()
                .map(|transform| transform.spatial())
                .collect(),
            image_bases,
            image_states,
            animations: layer
                .animations
                .iter()
                .map(|animation| animation.initial_runtime_state())
                .collect(),
        }
    }

    fn apply_animation_channels(
        &mut self,
        layer: &Layer,
        textures: &TextureList,
        animation_name: &[u8],
        frame: f32,
    ) -> Result<Option<RuntimeAnimationApplication>, crate::animation::AnimationError> {
        apply_runtime_layer_channels(
            layer,
            textures,
            animation_name,
            frame,
            &mut self.cast_transforms,
            &self.image_bases,
            &mut self.image_states,
            &mut self.animations,
        )
    }
}

impl ProjectRuntime {
    pub fn new(project: &Project) -> Result<Self, ReferenceRuntimeError> {
        let project_layers = project
            .scenes
            .iter()
            .enumerate()
            .map(|(scene_index, scene)| {
                scene
                    .layers
                    .iter()
                    .enumerate()
                    .map(|(layer_index, layer)| {
                        ProjectLayerRuntimeState::new(
                            ReferenceTarget {
                                scene_index,
                                layer_index,
                            },
                            layer,
                        )
                    })
                    .collect()
            })
            .collect();
        Ok(Self {
            project_layers,
            references: ReferenceRuntime::new(project)?,
        })
    }

    pub fn apply_layer_animation(
        &mut self,
        project: &Project,
        textures: &TextureList,
        target: ReferenceTarget,
        animation_name: &[u8],
        frame: f32,
    ) -> Result<Option<RuntimeAnimationTreeApplication>, crate::animation::AnimationError> {
        let layer = project
            .scenes
            .get(target.scene_index)
            .and_then(|scene| scene.layers.get(target.layer_index))
            .ok_or_else(|| {
                crate::animation::AnimationError(format!(
                    "SCN[{}]/LAYR[{}] is outside the project runtime",
                    target.scene_index, target.layer_index
                ))
            })?;
        let runtime_layer = self
            .project_layers
            .get_mut(target.scene_index)
            .and_then(|scene| scene.get_mut(target.layer_index))
            .expect("project runtime layers mirror the parsed project");
        let Some(application) =
            runtime_layer.apply_animation_channels(layer, textures, animation_name, frame)?
        else {
            return Ok(None);
        };
        let mut tree = RuntimeAnimationTreeApplication::default();
        tree.include_layer(application);

        let child_requests = child_animation_requests(
            layer,
            animation_name,
            frame,
            &self.references.plan,
            ReferenceLayerParent::ProjectLayer(target),
        );

        for (child_instance, child_animation_name, child_frame) in child_requests {
            tree.reference_requests += 1;
            if let Some(child) = self.references.apply_instance_animation(
                project,
                textures,
                child_instance,
                &child_animation_name,
                child_frame,
            )? {
                tree.include_tree(child);
            }
        }
        Ok(Some(tree))
    }

    pub fn apply_animation_set(
        &mut self,
        project: &Project,
        textures: &TextureList,
        scene_index: usize,
        animation_set_index: usize,
        frame: f32,
    ) -> Result<RuntimeAnimationTreeApplication, crate::animation::AnimationError> {
        let scene = project.scenes.get(scene_index).ok_or_else(|| {
            crate::animation::AnimationError(format!(
                "SCN[{scene_index}] is outside the project runtime"
            ))
        })?;
        let animation_set = scene
            .animation_sets
            .get(animation_set_index)
            .ok_or_else(|| {
                crate::animation::AnimationError(format!(
                    "SCN[{scene_index}]/ANMS[{animation_set_index}] is outside the scene"
                ))
            })?;
        self.apply_animation_set_definition(project, textures, scene_index, animation_set, frame)
    }

    fn apply_animation_set_definition(
        &mut self,
        project: &Project,
        textures: &TextureList,
        scene_index: usize,
        animation_set: &AnimationSetDefinition,
        frame: f32,
    ) -> Result<RuntimeAnimationTreeApplication, crate::animation::AnimationError> {
        let layer_count = self
            .project_layers
            .get(scene_index)
            .map(Vec::len)
            .ok_or_else(|| {
                crate::animation::AnimationError(format!(
                    "SCN[{scene_index}] is outside the project runtime"
                ))
            })?;
        let slots = animation_set
            .slots
            .iter()
            .take(layer_count)
            .map(|slot| (slot.is_enabled(), slot.animation_name.clone()))
            .collect::<Vec<_>>();
        let mut result = RuntimeAnimationTreeApplication::default();

        for (layer_index, (enabled, animation_name)) in slots.into_iter().enumerate() {
            self.project_layers[scene_index][layer_index].enabled = enabled;
            if animation_name.is_empty() {
                continue;
            }
            if let Some(application) = self.apply_layer_animation(
                project,
                textures,
                ReferenceTarget {
                    scene_index,
                    layer_index,
                },
                &animation_name,
                frame,
            )? {
                result.include_tree(application);
            }
        }
        Ok(result)
    }
}

#[allow(clippy::too_many_arguments)]
fn append_layer_references(
    project: &Project,
    source: ReferenceTarget,
    parent: ReferenceLayerParent,
    is_2d: bool,
    lineage: &[ReferenceTarget],
    plan: &mut ReferenceRuntimePlan,
    lineages: &mut Vec<Vec<ReferenceTarget>>,
) -> Result<(), ReferenceRuntimeError> {
    let layer = &project.scenes[source.scene_index].layers[source.layer_index];
    for (reference_node_index, reference) in layer.reference_by_node.iter().enumerate() {
        let Some(reference) = reference else {
            continue;
        };
        let Some(target) = project.resolve_reference(reference) else {
            plan.unresolved.push(UnresolvedReference {
                parent,
                reference_node_index,
                source_name: reference.source_name.clone(),
                layer_name: reference.layer_name.clone(),
            });
            continue;
        };

        if let Some(repeated_at) = lineage.iter().position(|entry| *entry == target) {
            let mut repeating_layers = lineage[repeated_at..].to_vec();
            repeating_layers.push(target);
            return Err(ReferenceRuntimeError { repeating_layers });
        }

        plan.instances.push(ReferenceLayerInstance {
            parent,
            reference_node_index,
            target,
            is_2d,
            flip_y: project.scenes[target.scene_index].layers[target.layer_index].is_2d() && !is_2d,
        });
        let mut child_lineage = lineage.to_vec();
        child_lineage.push(target);
        lineages.push(child_lineage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::animation::{AnimationDefinition, Key8, KeyData, Motion, Track};
    use crate::reference::ReferenceDefinition;
    use crate::scene::{Layer, NodeRecord, RawTransform, Scene};
    use crate::transform::SpatialTransform;

    use super::*;

    fn reference(source_name: &[u8], layer_name: &[u8], node_index: i32) -> ReferenceDefinition {
        ReferenceDefinition {
            source_name: source_name.to_vec(),
            layer_name: layer_name.to_vec(),
            animation_enabled: 0,
            animation_name: Vec::new(),
            default_frame: 0.0,
            node_index,
        }
    }

    fn layer(name: &[u8], is_2d: bool, references: Vec<Option<ReferenceDefinition>>) -> Layer {
        let count = references.len();
        Layer {
            name: name.to_vec(),
            flags: u32::from(!is_2d),
            animation_count: 0,
            animations: Vec::new(),
            field_23: Vec::new(),
            nodes: vec![
                NodeRecord {
                    name: None,
                    type_flags: Some(3),
                    parent_csli_cell_index: None,
                    first_child_index: -1,
                    next_sibling_index: -1,
                    field_a0: None,
                };
                count
            ],
            transforms: vec![RawTransform::Trs2(SpatialTransform::default()); count],
            image_by_node: vec![None; count],
            number_by_node: vec![None; count],
            reference_by_node: references,
            csli_by_node: vec![None; count],
            cast_attribute_lists: Vec::new(),
            cast_attribute_list_by_node: vec![None; count],
        }
    }

    fn project(layers: Vec<Layer>) -> Project {
        Project {
            name: Vec::new(),
            declared_scene_count: 1,
            camera: crate::camera::CameraDefinition::default(),
            scenes: vec![Scene {
                name: b"scene".to_vec(),
                declared_layer_count: layers.len() as u32,
                declared_animation_set_count: 0,
                width: 0.0,
                height: 0.0,
                layers,
                animation_sets: Vec::new(),
            }],
        }
    }

    #[test]
    fn repeated_targets_create_distinct_instances_and_nested_copies() {
        let project = project(vec![
            layer(
                b"root",
                true,
                vec![
                    Some(reference(b"scene", b"middle", 0)),
                    Some(reference(b"scene", b"middle", 1)),
                ],
            ),
            layer(
                b"middle",
                false,
                vec![Some(reference(b"scene", b"leaf", 0))],
            ),
            layer(b"leaf", true, Vec::new()),
        ]);

        let plan = project.build_reference_runtime_plan().unwrap();
        assert_eq!(plan.unresolved, Vec::new());
        assert_eq!(plan.instances.len(), 5);
        assert_eq!(plan.instances[0].target.layer_index, 1);
        assert_eq!(plan.instances[1].target.layer_index, 1);
        assert_eq!(plan.instances[2].target.layer_index, 2);
        assert_eq!(
            plan.instances[2].parent,
            ReferenceLayerParent::ProjectLayer(ReferenceTarget {
                scene_index: 0,
                layer_index: 1,
            })
        );
        assert_eq!(
            plan.instances[3].parent,
            ReferenceLayerParent::ReferenceInstance(0)
        );
        assert_eq!(
            plan.instances[4].parent,
            ReferenceLayerParent::ReferenceInstance(1)
        );
        assert!(plan.instances[0].is_2d);
        assert!(plan.instances[1].is_2d);
        assert!(!plan.instances[2].is_2d);
        assert!(plan.instances[3].is_2d);
        assert!(plan.instances[4].is_2d);
        assert!(!plan.instances[0].flip_y);
        assert!(!plan.instances[1].flip_y);
        assert!(plan.instances[2].flip_y);
        assert!(!plan.instances[3].flip_y);
        assert!(!plan.instances[4].flip_y);
    }

    #[test]
    fn unresolved_references_do_not_create_runtime_layers() {
        let project = project(vec![layer(
            b"root",
            true,
            vec![Some(reference(b"missing", b"layer", 0))],
        )]);
        let plan = project.build_reference_runtime_plan().unwrap();
        assert!(plan.instances.is_empty());
        assert_eq!(plan.unresolved.len(), 1);
    }

    #[test]
    fn cycles_are_reported_without_inventing_a_depth_limit() {
        let project = project(vec![layer(
            b"root",
            true,
            vec![Some(reference(b"scene", b"root", 0))],
        )]);
        let error = project.build_reference_runtime_plan().unwrap_err();
        assert_eq!(
            error.repeating_layers,
            vec![
                ReferenceTarget {
                    scene_index: 0,
                    layer_index: 0,
                },
                ReferenceTarget {
                    scene_index: 0,
                    layer_index: 0,
                }
            ]
        );
    }

    #[test]
    fn copied_layer_world_state_uses_the_owning_refcast_as_parent() {
        let instance = ReferenceLayerInstance {
            parent: ReferenceLayerParent::ProjectLayer(ReferenceTarget {
                scene_index: 0,
                layer_index: 0,
            }),
            reference_node_index: 0,
            target: ReferenceTarget {
                scene_index: 0,
                layer_index: 1,
            },
            is_2d: true,
            flip_y: false,
        };
        let parent = RuntimeWorldState {
            matrix: Affine3x4 {
                rows: [
                    [1.0, 0.0, 0.0, 10.0],
                    [0.0, 1.0, 0.0, 20.0],
                    [0.0, 0.0, 1.0, 30.0],
                ],
            },
            multiply_color: [200, 100, 50, 255],
            additive_color: [250, 20, 30, 40],
            visible: true,
            render_gate: true,
        };
        let mut local = ReferenceLayerLocalState::default();
        local.transform.translation = [2.0, 3.0, 4.0];
        local.transform.multiply_color = [128, 255, 0, 200];
        local.transform.additive_color = [10, 240, 20, 220];

        let world = instance.compose_world_state(parent, local);
        assert_eq!(world.matrix.rows[0][3], 12.0);
        assert_eq!(world.matrix.rows[1][3], 23.0);
        assert_eq!(world.matrix.rows[2][3], 34.0);
        assert_eq!(world.multiply_color, [100, 100, 0, 200]);
        assert_eq!(world.additive_color, [255, 255, 50, 255]);
        assert!(world.visible);
        assert!(world.render_gate);

        local.enabled = false;
        let disabled = instance.compose_world_state(parent, local);
        assert!(!disabled.visible);
        assert!(!disabled.render_gate);

        local.enabled = true;
        let gated_parent = RuntimeWorldState {
            render_gate: false,
            ..parent
        };
        let gated = instance.compose_world_state(gated_parent, local);
        assert!(gated.visible);
        assert!(!gated.render_gate);
    }

    #[test]
    fn copied_layers_keep_independent_animation_frames_and_cast_transforms() {
        let mut project = project(vec![
            layer(
                b"root",
                true,
                vec![
                    Some(reference(b"scene", b"target", 0)),
                    Some(reference(b"scene", b"target", 1)),
                ],
            ),
            layer(b"target", true, vec![None]),
        ]);
        let animation = AnimationDefinition {
            name: b"move".to_vec(),
            flags: 0,
            declared_motion_count: 1,
            duration: 10,
            motions: vec![Motion {
                target: 0,
                tracks: vec![Track {
                    target: 0,
                    key_count: 1,
                    format: 0x10,
                    range_start: 0,
                    range_end: 10,
                    keys: KeyData::Key8F32(vec![Key8 {
                        frame: 0,
                        value: 25.0,
                    }]),
                }],
            }],
        };
        project.scenes[0].layers[1].animation_count = 1;
        project.scenes[0].layers[1].animations.push(animation);

        let plan = project.build_reference_runtime_plan().unwrap();
        let mut first = ReferenceLayerRuntimeState::new(&project, &plan, 0).unwrap();
        let second = ReferenceLayerRuntimeState::new(&project, &plan, 1).unwrap();
        assert_eq!(
            first
                .apply_animation_common_channels(&project, &plan, b"move", 7.0)
                .unwrap(),
            Some(1)
        );
        assert_eq!(first.animations[0].frame, 7.0);
        assert_eq!(first.animations[0].duration, 10.0);
        assert_eq!(first.animations[0].flags, 9);
        assert_eq!(first.cast_transforms[0].translation[0], 25.0);
        assert_eq!(second.animations[0].frame, 0.0);
        assert_eq!(second.cast_transforms[0].translation[0], 0.0);
        assert_eq!(
            first
                .apply_animation_common_channels(&project, &plan, b"missing", 4.0)
                .unwrap(),
            None
        );
    }

    #[test]
    fn channel_23_recursively_applies_the_named_child_instance_animation() {
        let mut middle_reference = reference(b"scene", b"leaf", 0);
        middle_reference.animation_enabled = 1;
        middle_reference.animation_name = b"leaf_animation".to_vec();
        middle_reference.default_frame = 8.0;
        let mut project = project(vec![
            layer(b"root", true, vec![Some(reference(b"scene", b"middle", 0))]),
            layer(b"middle", true, vec![Some(middle_reference)]),
            layer(b"leaf", true, vec![None]),
        ]);
        project.scenes[0].layers[1].animation_count = 1;
        project.scenes[0].layers[1]
            .animations
            .push(AnimationDefinition {
                name: b"drive_reference".to_vec(),
                flags: 0,
                declared_motion_count: 1,
                duration: 10,
                motions: vec![Motion {
                    target: 0,
                    tracks: vec![Track {
                        target: 23,
                        key_count: 1,
                        format: 0x13,
                        range_start: 0,
                        range_end: 10,
                        keys: KeyData::Key20F32(vec![crate::animation::Key20 {
                            frame: 0,
                            value: 4.0,
                            mode: 0,
                            slope_in: 0.0,
                            slope_out: 0.0,
                        }]),
                    }],
                }],
            });
        project.scenes[0].layers[2].animation_count = 1;
        project.scenes[0].layers[2]
            .animations
            .push(AnimationDefinition {
                name: b"leaf_animation".to_vec(),
                flags: 0,
                declared_motion_count: 1,
                duration: 10,
                motions: vec![Motion {
                    target: 0,
                    tracks: vec![Track {
                        target: 0,
                        key_count: 1,
                        format: 0x10,
                        range_start: 0,
                        range_end: 10,
                        keys: KeyData::Key8F32(vec![Key8 {
                            frame: 0,
                            value: 99.0,
                        }]),
                    }],
                }],
            });

        let mut runtime = ReferenceRuntime::new(&project).unwrap();
        let nested_leaf = runtime
            .plan
            .instances
            .iter()
            .position(|instance| {
                instance.parent == ReferenceLayerParent::ReferenceInstance(0)
                    && instance.target.layer_index == 2
            })
            .unwrap();
        let top_level_leaf = runtime
            .plan
            .instances
            .iter()
            .position(|instance| {
                instance.parent
                    == ReferenceLayerParent::ProjectLayer(ReferenceTarget {
                        scene_index: 0,
                        layer_index: 1,
                    })
                    && instance.target.layer_index == 2
            })
            .unwrap();
        let textures = TextureList {
            declared_count: 0,
            textures: Vec::new(),
        };
        let application = runtime
            .apply_instance_animation(&project, &textures, 0, b"drive_reference", 3.0)
            .unwrap()
            .unwrap();
        assert_eq!(application.animated_layers, 2);
        assert_eq!(application.reference_requests, 1);
        assert_eq!(runtime.layers[0].animations[0].frame, 3.0);
        assert_eq!(runtime.layers[nested_leaf].animations[0].frame, 4.0);
        assert_eq!(
            runtime.layers[nested_leaf].cast_transforms[0].translation[0],
            99.0
        );
        assert_eq!(
            runtime.layers[top_level_leaf].cast_transforms[0].translation[0],
            0.0
        );
    }
}
