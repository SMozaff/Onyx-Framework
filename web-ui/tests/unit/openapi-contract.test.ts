import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const openapi = JSON.parse(
  readFileSync(join(process.cwd(), '../docs/api/openapi.json'), 'utf8'),
) as {
  openapi: string;
  paths: Record<string, unknown>;
  components: { schemas: Record<string, { properties?: Record<string, unknown>; required?: string[] }> };
};

describe('frozen OpenAPI contract', () => {
  it('contains every Team 6 endpoint', () => {
    expect(Object.keys(openapi.paths).sort()).toEqual([
      '/api/auth/login',
      '/api/auth/logout',
      '/api/command',
      '/api/events',
      '/api/query',
    ]);
  });

  it.each([
    ['CommandEnvelope', ['command_id', 'operation_id', 'command_type', 'schema_version', 'target', 'expected_version', 'expected_lifecycle_epoch', 'expected_authority_epoch', 'issued_at', 'vector_clock', 'correlation_id', 'causation_id', 'payload']],
    ['QueryEnvelope', ['query_id', 'query_type', 'schema_version', 'organization_id', 'filters']],
    ['DomainEventEnvelope', ['event_id', 'event_type', 'schema_version', 'aggregate_ref', 'aggregate_version', 'lifecycle_epoch', 'authority_epoch', 'operation_id', 'actor', 'occurred_at', 'recorded_at', 'vector_clock', 'correlation_id', 'causation_id', 'payload']],
  ] as const)('%s retains its frozen required fields', (schemaName, required) => {
    expect(openapi.components.schemas[schemaName].required).toEqual(required);
  });

  it('uses snake_case authentication fields', () => {
    const fields = Object.keys(openapi.components.schemas.LoginResponse.properties ?? {});
    expect(fields).toContain('access_token');
    expect(fields).toContain('refresh_token');
    expect(fields).not.toContain('accessToken');
  });
});
