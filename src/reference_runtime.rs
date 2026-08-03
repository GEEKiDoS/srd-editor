use std::fmt;

use crate::scene::{Project, ReferenceTarget};

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
        });
        let mut child_lineage = lineage.to_vec();
        child_lineage.push(target);
        lineages.push(child_lineage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        }
    }

    fn project(layers: Vec<Layer>) -> Project {
        Project {
            name: Vec::new(),
            declared_scene_count: 1,
            scenes: vec![Scene {
                name: b"scene".to_vec(),
                declared_layer_count: layers.len() as u32,
                declared_animation_set_count: 0,
                layers,
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
            layer(b"leaf", false, Vec::new()),
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
}
