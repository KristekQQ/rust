//! Declarative postprocessing chain. GPU allocation and command encoding live in
//! render/, so dependency/lifetime decisions can be tested without a GPU.
use crate::frame_graph::{CompiledGraph, Graph, Lifetime, PassId, ResourceDesc, ResourceId};

#[derive(Clone, Copy)]
pub struct EffectSpec {
    pub index: usize,
    pub name: &'static str,
    pub reads_depth: bool,
    pub half_trace: bool,
}

pub struct EffectStep {
    pub effect: usize,
    pub source: ResourceId,
    pub destination: ResourceId,
    pub trace: Option<ResourceId>,
    pub pass: PassId,
    pub trace_pass: Option<PassId>,
}

pub struct PostprocessPlan {
    pub graph: Graph,
    pub compiled: CompiledGraph,
    pub scene_color: ResourceId,
    pub output: ResourceId,
    pub steps: Vec<EffectStep>,
}

impl PostprocessPlan {
    pub fn new(
        effects: &[EffectSpec],
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        uniform_bytes: u64,
    ) -> Result<Self, String> {
        let mut graph = Graph::default();
        let texture = |width, height, format| ResourceDesc::Texture {
            width,
            height,
            layers: 1,
            samples: 1,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        };
        let scene_color = graph.resource(
            "scene.color",
            texture(width, height, format),
            Lifetime::External,
        );
        let depth = graph.resource(
            "scene.depth",
            texture(width, height, wgpu::TextureFormat::Depth32Float),
            Lifetime::External,
        );
        let output = if effects.is_empty() {
            scene_color
        } else {
            graph.resource(
                "view.completed.color",
                texture(width, height, format),
                Lifetime::External,
            )
        };
        let mut source = scene_color;
        let mut steps = Vec::with_capacity(effects.len());
        for (order, effect) in effects.iter().enumerate() {
            // A logical version per effect/view execution. Its caller-owned buffer
            // must not be overwritten before the corresponding submit (or it must
            // use a distinct buffer range when batching views).
            let uniform = graph.resource(
                format!("{}.uniform.current-view", effect.name),
                ResourceDesc::Buffer {
                    size: uniform_bytes,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                },
                Lifetime::External,
            );
            let destination = if order + 1 == effects.len() {
                output
            } else {
                graph.resource(
                    format!("{}.color", effect.name),
                    texture(width, height, format),
                    Lifetime::Transient,
                )
            };
            let mut reads = vec![source, uniform];
            if effect.reads_depth {
                reads.push(depth);
            }
            let (trace, trace_pass) = if effect.half_trace {
                let trace = graph.resource(
                    format!("{}.trace", effect.name),
                    texture(width.div_ceil(2), height.div_ceil(2), format),
                    Lifetime::Transient,
                );
                let pass = graph.pass(format!("{}.trace", effect.name), &reads, &[trace], false)?;
                reads.push(trace);
                (Some(trace), Some(pass))
            } else {
                (None, None)
            };
            let pass = graph.pass(
                format!("{}.compose", effect.name),
                &reads,
                &[destination],
                false,
            )?;
            steps.push(EffectStep {
                effect: effect.index,
                source,
                destination,
                trace,
                pass,
                trace_pass,
            });
            source = destination;
        }
        let compiled = graph.compile(&[output])?;
        // The encoder executes exactly this chain. Reject a planner/encoder drift
        // instead of silently treating the graph as diagnostic-only metadata.
        let expected: Vec<_> = steps
            .iter()
            .flat_map(|step| step.trace_pass.into_iter().chain(Some(step.pass)))
            .collect();
        if expected != compiled.ordered_passes {
            return Err("postprocess execution differs from compiled dependencies".into());
        }
        Ok(Self {
            graph,
            compiled,
            scene_color,
            output,
            steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan(effects: &[EffectSpec]) -> PostprocessPlan {
        PostprocessPlan::new(effects, 191, 203, wgpu::TextureFormat::Rgba8Unorm, 416).unwrap()
    }
    #[test]
    fn postprocess_plan_half_ssr_needs_only_a_quarter_size_trace() {
        let p = plan(&[EffectSpec {
            index: 1,
            name: "ssr",
            reads_depth: true,
            half_trace: true,
        }]);
        assert_eq!(p.compiled.slots.len(), 1);
        assert!(matches!(
            p.compiled.slots[0],
            ResourceDesc::Texture {
                width: 96,
                height: 102,
                ..
            }
        ));
        assert_eq!(p.compiled.ordered_passes.len(), 2);
        assert_eq!(p.compiled.resource_slots[p.output.0], None);
    }
    #[test]
    fn postprocess_plan_aliases_long_color_chains_without_read_write_feedback() {
        let effects: Vec<_> = (0..6)
            .map(|index| EffectSpec {
                index,
                name: "color",
                reads_depth: false,
                half_trace: false,
            })
            .collect();
        let p = plan(&effects);
        assert_eq!(p.compiled.slots.len(), 2);
        for step in &p.steps {
            if step.source != p.scene_color && step.destination != p.output {
                assert_ne!(
                    p.compiled.resource_slots[step.source.0],
                    p.compiled.resource_slots[step.destination.0]
                );
            }
        }
    }
}
