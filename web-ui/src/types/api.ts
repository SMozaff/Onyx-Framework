/**
 * Frozen OpenAPI 1.0 type surface derived from `docs/api/openapi.json`.
 *
 * This file follows the structural shape emitted by openapi-typescript while
 * reusing Team 6's canonical envelope interfaces as the single source inside
 * the web client. Regenerate from the frozen JSON during CI and compare this
 * file before accepting an API contract change.
 */
import type { CommandEnvelope, CommandError, CommandResult } from './command';
import type { DomainEventEnvelope } from './events';
import type { QueryEnvelope, QueryResponse } from './query';

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: {
    id: string;
    username: string;
    organization_id: string;
  };
}

export interface LogoutRequest {
  refresh_token: string;
}

export interface paths {
  '/api/auth/login': {
    post: {
      requestBody: LoginRequest;
      responses: { 200: LoginResponse; 401: { error: CommandError } };
    };
  };
  '/api/auth/logout': {
    post: {
      requestBody: LogoutRequest;
      responses: { 200: { success: true }; 401: { error: CommandError } };
    };
  };
  '/api/command': {
    post: {
      requestBody: CommandEnvelope;
      responses: {
        200: CommandResult;
        400: { error: CommandError };
        401: { error: CommandError };
        403: { error: CommandError };
        404: { error: CommandError };
        409: { error: CommandError };
        429: { error: CommandError };
        500: { error: CommandError };
      };
    };
  };
  '/api/query': {
    get: {
      parameters: { query: { envelope: string } };
      decodedEnvelope: QueryEnvelope;
      responses: {
        200: QueryResponse;
        400: { error: CommandError };
        401: { error: CommandError };
        403: { error: CommandError };
        429: { error: CommandError };
        500: { error: CommandError };
      };
    };
  };
  '/api/events': {
    get: {
      parameters: { query: { token: string } };
      messages: DomainEventEnvelope;
      responses: { 101: never; 401: { error: CommandError } };
    };
  };
}

export interface components {
  schemas: {
    LoginRequest: LoginRequest;
    LoginResponse: LoginResponse;
    LogoutRequest: LogoutRequest;
    CommandEnvelope: CommandEnvelope;
    CommandResult: CommandResult;
    CommandError: CommandError;
    QueryEnvelope: QueryEnvelope;
    QueryResponse: QueryResponse;
    DomainEventEnvelope: DomainEventEnvelope;
  };
}
