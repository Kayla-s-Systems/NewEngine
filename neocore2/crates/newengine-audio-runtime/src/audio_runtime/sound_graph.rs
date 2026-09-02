#[derive(Clone, Debug)]
struct SoundGraphVoicePlan {
    clip_name: String,
    gain: f32,
    pitch: f32,
    label: String,
}

struct SoundGraphEvalContext<'a> {
    canonical: &'a str,
    seed: u64,
    scope_id: Option<u64>,
    parameters: &'a AudioParameterSet,
}

impl AudioRuntimeState {
    fn evaluate_sound_graph(
        &mut self,
        canonical: &str,
        graph: &newengine_asset_format_nef8::YsncdSoundGraph,
        parameters: &AudioParameterSet,
        seed: u64,
        scope_id: Option<u64>,
    ) -> Result<Vec<SoundGraphVoicePlan>, String> {
        let nodes = graph
            .nodes
            .iter()
            .map(|node| (node.id().trim().to_ascii_lowercase(), node))
            .collect::<HashMap<_, _>>();
        let ctx = SoundGraphEvalContext {
            canonical,
            seed,
            scope_id: scope_id.filter(|id| *id != 0),
            parameters,
        };
        let mut stack = Vec::<String>::new();
        let mut staged_sequences = HashMap::<String, u64>::new();
        let mut plans = self.eval_sound_graph_voice_node(
            graph.root.as_str(),
            &nodes,
            &ctx,
            &mut stack,
            &mut staged_sequences,
        )?;
        if plans.is_empty() {
            return Err(format!(
                "YSNCD SoundGraph '{}' evaluated to no logical voices",
                canonical
            ));
        }
        if plans.len() > 64 {
            return Err(format!(
                "YSNCD SoundGraph '{}' emitted {} voices; max is 64",
                canonical,
                plans.len()
            ));
        }
        for plan in &mut plans {
            plan.gain = sanitize_gain(plan.gain);
            plan.pitch = sanitize_speed(plan.pitch);
        }
        for (key, next) in staged_sequences {
            self.sound_graph_sequences.insert(key, next);
        }
        Ok(plans)
    }

    fn eval_sound_graph_voice_node(
        &mut self,
        node_id: &str,
        nodes: &HashMap<String, &newengine_asset_format_nef8::YsncdSoundGraphNode>,
        ctx: &SoundGraphEvalContext<'_>,
        stack: &mut Vec<String>,
        staged_sequences: &mut HashMap<String, u64>,
    ) -> Result<Vec<SoundGraphVoicePlan>, String> {
        let key = node_id.trim().to_ascii_lowercase();
        if stack.len() >= 64 {
            return Err("YSNCD SoundGraph runtime traversal exceeded depth 64".to_owned());
        }
        if stack.iter().any(|entry| entry == &key) {
            return Err(format!(
                "YSNCD SoundGraph runtime cycle detected at node '{}'",
                node_id
            ));
        }
        let node = nodes
            .get(&key)
            .copied()
            .ok_or_else(|| format!("YSNCD SoundGraph node '{}' does not resolve", node_id))?;
        if node.output_kind() != newengine_asset_format_nef8::YsncdSoundGraphValueKind::Voices {
            return Err(format!(
                "YSNCD SoundGraph node '{}' was evaluated as voices but produces scalar",
                node_id
            ));
        }
        stack.push(key.clone());
        let result = match node {
            newengine_asset_format_nef8::YsncdSoundGraphNode::Clip {
                id,
                clip,
                gain,
                pitch,
            } => Ok(vec![SoundGraphVoicePlan {
                clip_name: clip.trim().to_ascii_lowercase(),
                gain: *gain,
                pitch: *pitch,
                label: id.clone(),
            }]),
            newengine_asset_format_nef8::YsncdSoundGraphNode::Random { id, children } => {
                let total = children.iter().map(|child| child.weight).sum::<f32>();
                if !total.is_finite() || total <= 0.0 {
                    return Err(format!(
                        "YSNCD SoundGraph Random node '{}' has invalid weight total",
                        id
                    ));
                }
                let random = unit_f32(splitmix64(ctx.seed ^ stable_text_hash(id)));
                let mut cursor = random * total;
                let mut selected = children.last().expect("validated Random children");
                for child in children {
                    if cursor < child.weight {
                        selected = child;
                        break;
                    }
                    cursor -= child.weight;
                }
                self.eval_sound_graph_voice_node(&selected.node, nodes, ctx, stack, staged_sequences)
            }
            newengine_asset_format_nef8::YsncdSoundGraphNode::Sequence { id, children } => {
                let state_key = match ctx.scope_id {
                    Some(scope_id) => format!(
                        "{}#{}#object:{}",
                        ctx.canonical,
                        id.trim().to_ascii_lowercase(),
                        scope_id
                    ),
                    None => format!(
                        "{}#{}#global",
                        ctx.canonical,
                        id.trim().to_ascii_lowercase()
                    ),
                };
                let cursor = staged_sequences
                    .get(&state_key)
                    .copied()
                    .or_else(|| self.sound_graph_sequences.get(&state_key).copied())
                    .unwrap_or(0);
                let selected = children[(cursor as usize) % children.len()].clone();
                staged_sequences.insert(state_key, cursor.wrapping_add(1));
                self.eval_sound_graph_voice_node(&selected, nodes, ctx, stack, staged_sequences)
            }
            newengine_asset_format_nef8::YsncdSoundGraphNode::Switch {
                id,
                switch,
                cases,
                default,
            } => {
                let value = ctx
                    .parameters
                    .switches
                    .get(switch)
                    .map(String::as_str)
                    .unwrap_or("");
                let selected = cases
                    .iter()
                    .find(|(candidate, _)| candidate.eq_ignore_ascii_case(value))
                    .map(|(_, target)| target)
                    .or(default.as_ref())
                    .ok_or_else(|| {
                        format!(
                            "YSNCD SoundGraph Switch node '{}' has no case for switch '{}' value '{}' and no default",
                            id, switch, value
                        )
                    })?;
                self.eval_sound_graph_voice_node(selected, nodes, ctx, stack, staged_sequences)
            }
            newengine_asset_format_nef8::YsncdSoundGraphNode::Blend1d { id, input, points } => {
                let value = self.eval_sound_graph_scalar_node(input, nodes, ctx, stack)?;
                if points.len() == 1 || value <= points[0].value {
                    self.eval_sound_graph_voice_node(&points[0].node, nodes, ctx, stack, staged_sequences)
                } else if value >= points[points.len() - 1].value {
                    self.eval_sound_graph_voice_node(&points[points.len() - 1].node, nodes, ctx, stack, staged_sequences)
                } else {
                    let upper = points
                        .iter()
                        .position(|point| point.value >= value)
                        .ok_or_else(|| format!("YSNCD SoundGraph Blend1D node '{}' bracket failed", id))?;
                    let lower = upper.saturating_sub(1);
                    let a = &points[lower];
                    let b = &points[upper];
                    let span = b.value - a.value;
                    if !span.is_finite() || span <= 0.0 {
                        return Err(format!(
                            "YSNCD SoundGraph Blend1D node '{}' has invalid point span",
                            id
                        ));
                    }
                    let t = ((value - a.value) / span).clamp(0.0, 1.0);
                    if t <= 1.0e-6 {
                        self.eval_sound_graph_voice_node(&a.node, nodes, ctx, stack, staged_sequences)
                    } else if t >= 1.0 - 1.0e-6 {
                        self.eval_sound_graph_voice_node(&b.node, nodes, ctx, stack, staged_sequences)
                    } else {
                        let mut left = self.eval_sound_graph_voice_node(&a.node, nodes, ctx, stack, staged_sequences)?;
                        let mut right = self.eval_sound_graph_voice_node(&b.node, nodes, ctx, stack, staged_sequences)?;
                        for plan in &mut left {
                            plan.gain *= 1.0 - t;
                            plan.label = format!("{}:a:{}", id, plan.label);
                        }
                        for plan in &mut right {
                            plan.gain *= t;
                            plan.label = format!("{}:b:{}", id, plan.label);
                        }
                        left.extend(right);
                        Ok(left)
                    }
                }
            }
            newengine_asset_format_nef8::YsncdSoundGraphNode::Layer { id, children } => {
                let mut output = Vec::new();
                for child in children {
                    let mut child_output =
                        self.eval_sound_graph_voice_node(&child.node, nodes, ctx, stack, staged_sequences)?;
                    for plan in &mut child_output {
                        plan.gain *= child.gain;
                        plan.pitch *= child.pitch;
                        plan.label = format!("{}:{}", id, plan.label);
                    }
                    output.extend(child_output);
                    if output.len() > 64 {
                        return Err(format!(
                            "YSNCD SoundGraph Layer node '{}' exceeds 64 emitted voices",
                            id
                        ));
                    }
                }
                Ok(output)
            }
            newengine_asset_format_nef8::YsncdSoundGraphNode::Parameter { .. }
            | newengine_asset_format_nef8::YsncdSoundGraphNode::Envelope { .. } => unreachable!(
                "typed graph validation prevents scalar node on voice path"
            ),
        };
        stack.pop();
        result
    }

    fn eval_sound_graph_scalar_node(
        &mut self,
        node_id: &str,
        nodes: &HashMap<String, &newengine_asset_format_nef8::YsncdSoundGraphNode>,
        ctx: &SoundGraphEvalContext<'_>,
        stack: &mut Vec<String>,
    ) -> Result<f32, String> {
        let key = node_id.trim().to_ascii_lowercase();
        if stack.len() >= 64 {
            return Err("YSNCD SoundGraph scalar traversal exceeded depth 64".to_owned());
        }
        if stack.iter().any(|entry| entry == &key) {
            return Err(format!(
                "YSNCD SoundGraph runtime cycle detected at scalar node '{}'",
                node_id
            ));
        }
        let node = nodes
            .get(&key)
            .copied()
            .ok_or_else(|| format!("YSNCD SoundGraph scalar node '{}' does not resolve", node_id))?;
        if node.output_kind() != newengine_asset_format_nef8::YsncdSoundGraphValueKind::Scalar {
            return Err(format!(
                "YSNCD SoundGraph node '{}' was evaluated as scalar but produces voices",
                node_id
            ));
        }
        stack.push(key);
        let result = match node {
            newengine_asset_format_nef8::YsncdSoundGraphNode::Parameter {
                parameter,
                default,
                min,
                max,
                ..
            } => Ok(ctx
                .parameters
                .scalars
                .get(parameter)
                .copied()
                .unwrap_or(*default)
                .clamp(*min, *max)),
            newengine_asset_format_nef8::YsncdSoundGraphNode::Envelope { id, input, points } => {
                let value = self.eval_sound_graph_scalar_node(input, nodes, ctx, stack)?;
                if points.len() == 1 || value <= points[0][0] {
                    Ok(points[0][1])
                } else if value >= points[points.len() - 1][0] {
                    Ok(points[points.len() - 1][1])
                } else {
                    let upper = points
                        .iter()
                        .position(|point| point[0] >= value)
                        .ok_or_else(|| format!("YSNCD SoundGraph Envelope node '{}' bracket failed", id))?;
                    let lower = upper.saturating_sub(1);
                    let [x0, y0] = points[lower];
                    let [x1, y1] = points[upper];
                    let span = x1 - x0;
                    if !span.is_finite() || span <= 0.0 {
                        return Err(format!(
                            "YSNCD SoundGraph Envelope node '{}' has invalid point span",
                            id
                        ));
                    }
                    let t = ((value - x0) / span).clamp(0.0, 1.0);
                    Ok(y0 + (y1 - y0) * t)
                }
            }
            _ => unreachable!("typed graph validation prevents voice node on scalar path"),
        };
        stack.pop();
        result
    }
}

#[cfg(test)]
mod sound_graph_runtime_tests {
    include!("sound_graph/tests.rs");
}
