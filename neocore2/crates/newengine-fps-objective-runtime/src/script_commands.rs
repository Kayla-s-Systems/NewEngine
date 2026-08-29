use newengine_ecs::World;
use newengine_gameplay_script_api::GameplayCommandBuffer;
use newengine_gameplay_script_runtime::GameplayCommandExecutor;

pub(crate) fn execute_policy_commands(
    world: &mut World,
    executor: &GameplayCommandExecutor,
    commands: &GameplayCommandBuffer,
    label: &str,
) -> Result<(), String> {
    if commands.commands.is_empty() {
        return Ok(());
    }
    let receipt = executor.execute(world, commands)?;
    newengine_ulog_api::ulog::info!(
        "fps scripted command transaction committed label='{}' tx='{}' commands={} damage={:.2} items={} spawned={} objectives={} effects={}",
        label,
        receipt.transaction_id,
        receipt.applied_commands,
        receipt.total_damage,
        receipt.items_given,
        receipt.spawned_entities.len(),
        receipt.objectives_touched.len(),
        receipt.effects_enqueued,
    );
    Ok(())
}
