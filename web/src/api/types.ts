export interface FdmlDocument {
  metadata?: Metadata;
  system?: System;
  entities: Entity[];
  actions: Action[];
  features: Feature[];
  flows: Flow[];
  constraints: Constraint[];
  traceability: Traceability[];
  generation_rules: GenerationRule[];

  // FDML 1.5: Architectural Level
  contours: Contour[];
  systems: SystemEntry[];
  integrations: Integration[];
  cross_flows: CrossFlow[];
  shared_entities: SharedEntity[];
}

export interface Metadata {
  version: string;
  author?: string;
  description?: string;
  created?: string;
  updated?: string;
}

export interface System {
  id: string;
  name: string;
  description?: string;
  components: string[];
  relationships: Relationship[];
}

export interface Relationship {
  from: string;
  to: string;
  type: string;
  description?: string;
}

export interface Entity {
  id: string;
  name?: string;
  description?: string;
  fields: Field[];
  relationships?: EntityRelationship[];
}

export interface Field {
  name: string;
  type: string;
  description?: string;
  required?: boolean;
  default?: unknown;
  constraints?: FieldConstraint[];
}

export interface FieldConstraint {
  type: string;
  value?: unknown;
  message?: string;
}

export interface EntityRelationship {
  entity: string;
  type: string;
  description?: string;
}

export interface Action {
  id: string;
  name?: string;
  description?: string;
  input?: ActionData;
  output?: ActionData;
  side_effects?: string[];
  preconditions?: string[];
  postconditions?: string[];
}

export interface ActionData {
  entity?: string;
  fields?: string[];
  description?: string;
}

export interface Feature {
  id: string;
  title: string;
  description?: string;
  scenarios: Scenario[];
  acceptance_criteria?: string[];
  dependencies?: string[];
}

export interface Scenario {
  id: string;
  title: string;
  description?: string;
  given: string[];
  when: string[];
  then: string[];
}

export interface Flow {
  id: string;
  name: string;
  description?: string;
  steps: FlowStep[];
}

export interface FlowStep {
  id: string;
  action: string;
  description?: string;
  conditions?: string[];
}

export interface Constraint {
  id: string;
  name: string;
  description?: string;
  type: string;
  rule: string;
  entities?: string[];
  actions?: string[];
}

export interface Traceability {
  from: string;
  to: string;
  relation: string;
  description?: string;
}

export interface GenerationRule {
  id: string;
  name: string;
  description?: string;
  triggers: string[];
  generates: string[];
  template?: string;
}

// --- FDML 1.5: Architectural Level ---

export interface Contour {
  id: string;
  name: string;
  description?: string;
  trust_level?: 'public' | 'internal' | 'restricted' | 'critical';
}

export type SystemType = 'frontend' | 'gateway' | 'service' | 'worker' | 'database' | 'queue' | 'storage' | 'external';

export interface SystemEntry {
  id: string;
  name: string;
  description?: string;
  type: SystemType;
  technology?: string;
  contour?: string;
  spec?: string;
  owner?: string;
  components: string[];
  relationships: Relationship[];
}

export type IntegrationType = 'http' | 'grpc' | 'queue' | 'event' | 'shared_db' | 'websocket' | 'file' | 'graphql';

export interface Integration {
  id: string;
  from: string;
  to: string;
  type: IntegrationType;
  protocol?: string;
  description?: string;
  async?: boolean;
  endpoints: IntegrationEndpoint[];
  channels: string[];
  data_entities: string[];
}

export interface IntegrationEndpoint {
  method: string;
  path: string;
  description?: string;
}

export interface CrossFlow {
  id: string;
  name: string;
  description?: string;
  trigger?: string;
  steps: CrossFlowStep[];
}

export interface CrossFlowStep {
  id: string;
  system: string;
  action?: string;
  description?: string;
  integration?: string;
  on_success?: string;
  on_failure?: string;
}

export interface SharedEntity {
  entity: string;
  description?: string;
  canonical_system?: string;
  contexts: SharedEntityContext[];
}

export interface SharedEntityContext {
  system: string;
  entity_id: string;
  role?: 'source' | 'replica' | 'projection' | 'cache';
  fields: string[];
  notes?: string;
}
