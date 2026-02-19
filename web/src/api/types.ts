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
