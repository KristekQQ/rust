//! GPU-independent resource scheduling. Logical resources have one writer;
//! create a new handle for every version, eye, or subresource being scheduled.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ResourceId(pub usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PassId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifetime {
    Transient,
    Persistent,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceDesc {
    Texture {
        width: u32,
        height: u32,
        layers: u32,
        samples: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    },
    Buffer {
        size: u64,
        usage: wgpu::BufferUsages,
    },
}

#[derive(Debug)]
struct Resource {
    name: String,
    desc: ResourceDesc,
    lifetime: Lifetime,
}
#[derive(Debug)]
struct Pass {
    name: String,
    reads: Vec<ResourceId>,
    writes: Vec<ResourceId>,
    side_effect: bool,
}

#[derive(Debug, Default)]
pub struct Graph {
    resources: Vec<Resource>,
    passes: Vec<Pass>,
}

/// Inclusive indices in `CompiledGraph::ordered_passes`. Outputs stay live
/// through the end of the graph, so their slots cannot be reused prematurely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLifetime {
    pub first_use: usize,
    pub last_use: usize,
}

#[derive(Debug)]
pub struct CompiledGraph {
    pub ordered_passes: Vec<PassId>,
    /// None for unused resources and caller-owned persistent/external resources.
    pub resource_slots: Vec<Option<usize>>,
    pub slots: Vec<ResourceDesc>,
    pub resource_lifetimes: Vec<Option<ResourceLifetime>>,
    pub pruned_pass_count: usize,
}

impl Graph {
    pub fn resource(
        &mut self,
        name: impl Into<String>,
        desc: ResourceDesc,
        lifetime: Lifetime,
    ) -> ResourceId {
        let id = ResourceId(self.resources.len());
        self.resources.push(Resource {
            name: name.into(),
            desc,
            lifetime,
        });
        id
    }

    pub fn resource_name(&self, id: ResourceId) -> Option<&str> {
        self.resources
            .get(id.0)
            .map(|resource| resource.name.as_str())
    }

    pub fn pass_name(&self, id: PassId) -> Option<&str> {
        self.passes.get(id.0).map(|pass| pass.name.as_str())
    }

    pub fn pass(
        &mut self,
        name: impl Into<String>,
        reads: &[ResourceId],
        writes: &[ResourceId],
        side_effect: bool,
    ) -> Result<PassId, String> {
        let name = name.into();
        for id in reads.iter().chain(writes) {
            if id.0 >= self.resources.len() {
                return Err(format!("pass {name}: unknown resource {}", id.0));
            }
        }
        if reads.iter().any(|id| writes.contains(id)) {
            return Err(format!(
                "pass {name}: reading and writing the same resource is unsupported"
            ));
        }
        if writes
            .iter()
            .enumerate()
            .any(|(i, id)| writes[..i].contains(id))
        {
            return Err(format!("pass {name}: duplicate resource write"));
        }
        let id = PassId(self.passes.len());
        self.passes.push(Pass {
            name,
            reads: reads.to_vec(),
            writes: writes.to_vec(),
            side_effect,
        });
        Ok(id)
    }

    pub fn compile(&self, outputs: &[ResourceId]) -> Result<CompiledGraph, String> {
        for resource in &self.resources {
            let valid = match resource.desc {
                ResourceDesc::Texture {
                    width,
                    height,
                    layers,
                    samples,
                    ..
                } => width > 0 && height > 0 && layers > 0 && samples.is_power_of_two(),
                ResourceDesc::Buffer { size, .. } => size > 0,
            };
            if !valid {
                return Err(format!(
                    "resource {}: invalid dimensions or size",
                    resource.name
                ));
            }
        }
        let mut writers = vec![None; self.resources.len()];
        for (index, pass) in self.passes.iter().enumerate() {
            for id in &pass.writes {
                if writers[id.0].replace(index).is_some() {
                    return Err(format!(
                        "resource {}: multiple writers",
                        self.resources[id.0].name
                    ));
                }
            }
        }
        let mut dependencies = vec![BTreeSet::new(); self.passes.len()];
        for (index, pass) in self.passes.iter().enumerate() {
            for id in &pass.reads {
                if let Some(writer) = writers[id.0] {
                    dependencies[index].insert(writer);
                } else if self.resources[id.0].lifetime == Lifetime::Transient {
                    return Err(format!(
                        "resource {}: uninitialized transient read",
                        self.resources[id.0].name
                    ));
                }
            }
        }
        // Validate even dead branches; an invalid graph must not silently become
        // valid just because a diagnostic or requested output changed.
        let all_ordered = topological_order(&dependencies)?;
        let mut needed = vec![false; self.passes.len()];
        let mut pending: Vec<usize> = self
            .passes
            .iter()
            .enumerate()
            .filter_map(|(index, pass)| pass.side_effect.then_some(index))
            .collect();
        for id in outputs {
            let resource = self
                .resources
                .get(id.0)
                .ok_or_else(|| format!("unknown output resource {}", id.0))?;
            if let Some(writer) = writers[id.0] {
                pending.push(writer);
            } else if resource.lifetime == Lifetime::Transient {
                return Err(format!("resource {}: uninitialized output", resource.name));
            }
        }
        while let Some(index) = pending.pop() {
            if !needed[index] {
                needed[index] = true;
                pending.extend(dependencies[index].iter().copied());
            }
        }
        let ordered_passes: Vec<PassId> = all_ordered
            .into_iter()
            .filter(|index| needed[*index])
            .map(PassId)
            .collect();
        let mut resource_lifetimes: Vec<Option<ResourceLifetime>> =
            vec![None; self.resources.len()];
        for (position, id) in ordered_passes.iter().enumerate() {
            let pass = &self.passes[id.0];
            for resource in pass.reads.iter().chain(&pass.writes) {
                resource_lifetimes[resource.0]
                    .get_or_insert(ResourceLifetime {
                        first_use: position,
                        last_use: position,
                    })
                    .last_use = position;
            }
        }
        for id in outputs {
            if let Some(range) = &mut resource_lifetimes[id.0] {
                range.last_use = ordered_passes.len().saturating_sub(1);
            }
        }
        let mut transient: Vec<usize> = self
            .resources
            .iter()
            .enumerate()
            .filter_map(|(index, resource)| {
                (resource.lifetime == Lifetime::Transient && resource_lifetimes[index].is_some())
                    .then_some(index)
            })
            .collect();
        transient.sort_by_key(|index| (resource_lifetimes[*index].unwrap().first_use, *index));
        let mut slots = Vec::new();
        let mut slot_last_use = Vec::new();
        let mut resource_slots = vec![None; self.resources.len()];
        for index in transient {
            let range = resource_lifetimes[index].unwrap();
            let desc = &self.resources[index].desc;
            let slot = slots
                .iter()
                .enumerate()
                .position(|(slot, candidate)| {
                    candidate == desc && slot_last_use[slot] < range.first_use
                })
                .unwrap_or_else(|| {
                    slots.push(desc.clone());
                    slot_last_use.push(0);
                    slots.len() - 1
                });
            resource_slots[index] = Some(slot);
            slot_last_use[slot] = range.last_use;
        }
        Ok(CompiledGraph {
            pruned_pass_count: self.passes.len() - ordered_passes.len(),
            ordered_passes,
            resource_slots,
            slots,
            resource_lifetimes,
        })
    }

    /// Human/AI diagnostics without any device handles or GPU side effects.
    pub fn debug_summary(&self, plan: &CompiledGraph) -> serde_json::Value {
        serde_json::json!({
            "passes": plan.ordered_passes.iter().map(|id| self.pass_name(*id)).collect::<Vec<_>>(),
            "prunedPassCount": plan.pruned_pass_count,
            "slots": plan.slots.iter().map(|desc| format!("{desc:?}")).collect::<Vec<_>>(),
            "resources": self.resources.iter().enumerate().map(|(index, resource)| {
                serde_json::json!({
                    "name": resource.name,
                    "lifetime": format!("{:?}", resource.lifetime),
                    "slot": plan.resource_slots[index],
                    "firstUse": plan.resource_lifetimes[index].map(|range| range.first_use),
                    "lastUse": plan.resource_lifetimes[index].map(|range| range.last_use),
                })
            }).collect::<Vec<_>>(),
        })
    }
}

fn topological_order(dependencies: &[BTreeSet<usize>]) -> Result<Vec<usize>, String> {
    let mut remaining: Vec<usize> = dependencies.iter().map(BTreeSet::len).collect();
    let mut dependents = vec![Vec::new(); dependencies.len()];
    for (pass, predecessors) in dependencies.iter().enumerate() {
        for predecessor in predecessors {
            dependents[*predecessor].push(pass);
        }
    }
    let mut ready: BTreeSet<usize> = remaining
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect();
    let mut ordered = Vec::new();
    while let Some(index) = ready.pop_first() {
        ordered.push(index);
        for dependent in &dependents[index] {
            remaining[*dependent] -= 1;
            if remaining[*dependent] == 0 {
                ready.insert(*dependent);
            }
        }
    }
    if ordered.len() != dependencies.len() {
        return Err("frame graph contains a dependency cycle".into());
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(graph: &mut Graph, name: &str, lifetime: Lifetime) -> ResourceId {
        graph.resource(
            name,
            ResourceDesc::Buffer {
                size: 256,
                usage: wgpu::BufferUsages::STORAGE,
            },
            lifetime,
        )
    }

    #[test]
    fn frame_graph_orders_out_of_order_dependencies_and_prunes() {
        let mut graph = Graph::default();
        let source = buffer(&mut graph, "source", Lifetime::External);
        let intermediate = buffer(&mut graph, "intermediate", Lifetime::Transient);
        let result = buffer(&mut graph, "result", Lifetime::Transient);
        let consumer = graph
            .pass("consumer", &[intermediate], &[result], false)
            .unwrap();
        graph.pass("unused", &[], &[], false).unwrap();
        let producer = graph
            .pass("producer", &[source], &[intermediate], false)
            .unwrap();
        let effect = graph.pass("effect", &[source], &[], true).unwrap();
        let plan = graph.compile(&[result]).unwrap();
        assert_eq!(plan.ordered_passes, [producer, consumer, effect]);
        assert_eq!(plan.pruned_pass_count, 1);
        assert_eq!(graph.debug_summary(&plan)["passes"][0], "producer");
    }

    #[test]
    fn frame_graph_rejects_cycles_and_hazards_even_in_dead_branches() {
        let mut graph = Graph::default();
        let a = buffer(&mut graph, "a", Lifetime::Transient);
        let b = buffer(&mut graph, "b", Lifetime::Transient);
        assert!(graph.pass("in place", &[a], &[a], false).is_err());
        assert!(graph.pass("invalid", &[ResourceId(9)], &[], false).is_err());
        assert!(graph.pass("duplicate", &[], &[a, a], false).is_err());
        graph.pass("a", &[b], &[a], false).unwrap();
        graph.pass("b", &[a], &[b], false).unwrap();
        assert!(graph.compile(&[]).unwrap_err().contains("cycle"));
    }

    #[test]
    fn frame_graph_rejects_uninitialized_reads_and_multiple_writers() {
        let mut graph = Graph::default();
        let a = buffer(&mut graph, "a", Lifetime::Transient);
        assert!(graph.compile(&[a]).unwrap_err().contains("uninitialized"));
        graph.pass("read", &[a], &[], true).unwrap();
        assert!(graph.compile(&[]).unwrap_err().contains("uninitialized"));
        graph.pass("write", &[], &[a], false).unwrap();
        graph.pass("second write", &[], &[a], false).unwrap();
        assert!(graph.compile(&[]).unwrap_err().contains("multiple writers"));
    }

    #[test]
    fn frame_graph_aliases_only_nonoverlapping_compatible_transients() {
        let mut graph = Graph::default();
        let a = buffer(&mut graph, "a", Lifetime::Transient);
        let b = buffer(&mut graph, "b", Lifetime::Transient);
        let c = buffer(&mut graph, "c", Lifetime::Transient);
        let persistent = buffer(&mut graph, "persistent", Lifetime::Persistent);
        let external = buffer(&mut graph, "external", Lifetime::External);
        graph
            .pass("first", &[external, persistent], &[a], true)
            .unwrap();
        graph.pass("overlap", &[a], &[b], true).unwrap();
        graph.pass("later", &[b], &[c], true).unwrap();
        let plan = graph.compile(&[c]).unwrap();
        assert_ne!(plan.resource_slots[a.0], plan.resource_slots[b.0]);
        assert_eq!(plan.resource_slots[a.0], plan.resource_slots[c.0]);
        assert_eq!(plan.resource_slots[external.0], None);
        assert_eq!(plan.resource_slots[persistent.0], None);
        assert_eq!(plan.slots.len(), 2);
    }

    #[test]
    fn frame_graph_keeps_outputs_alive_and_separates_descriptors() {
        let mut graph = Graph::default();
        let a = buffer(&mut graph, "a", Lifetime::Transient);
        let b = buffer(&mut graph, "b", Lifetime::Transient);
        let c = graph.resource(
            "different",
            ResourceDesc::Buffer {
                size: 512,
                usage: wgpu::BufferUsages::STORAGE,
            },
            Lifetime::Transient,
        );
        graph.pass("a", &[], &[a], false).unwrap();
        graph.pass("b", &[], &[b], true).unwrap();
        graph.pass("c", &[], &[c], true).unwrap();
        let plan = graph.compile(&[a]).unwrap();
        assert_ne!(plan.resource_slots[a.0], plan.resource_slots[b.0]);
        assert_ne!(plan.resource_slots[b.0], plan.resource_slots[c.0]);
        assert_eq!(plan.resource_lifetimes[a.0].unwrap().last_use, 2);
    }

    #[test]
    fn frame_graph_validates_resource_dimensions() {
        let mut graph = Graph::default();
        graph.resource(
            "invalid",
            ResourceDesc::Texture {
                width: 0,
                height: 1,
                layers: 1,
                samples: 1,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            },
            Lifetime::External,
        );
        assert!(graph
            .compile(&[])
            .unwrap_err()
            .contains("invalid dimensions"));
        let mut graph = Graph::default();
        graph.resource(
            "zero",
            ResourceDesc::Buffer {
                size: 0,
                usage: wgpu::BufferUsages::STORAGE,
            },
            Lifetime::External,
        );
        assert!(graph.compile(&[]).is_err());
    }
}
