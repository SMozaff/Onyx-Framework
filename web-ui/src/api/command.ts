import { apiClient } from './client';
import type { CommandEnvelope, CommandInput, CommandResult } from '../types/command';

export function buildCommandEnvelope<T>(input: CommandInput<T>): CommandEnvelope<T> {
  return {
    command_id: crypto.randomUUID(),
    operation_id: crypto.randomUUID(),
    command_type: input.command_type,
    schema_version: '1.0',
    target: input.target,
    expected_version: input.expected_version,
    expected_lifecycle_epoch: input.expected_lifecycle_epoch,
    expected_authority_epoch: input.expected_authority_epoch,
    issued_at: new Date().toISOString(),
    vector_clock: { entries: {} },
    correlation_id: crypto.randomUUID(),
    causation_id: null,
    payload: input.payload,
  };
}

export async function executeCommand<TPayload, TResult = unknown>(input: CommandInput<TPayload>): Promise<CommandResult<TResult>> {
  const envelope = buildCommandEnvelope(input);
  const response = await apiClient.post<CommandResult<TResult>>('/api/command', envelope);
  return response.data;
}
