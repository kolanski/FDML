import type { ReactNode } from 'react';
import type { FdmlDocument } from '../api/types';
import type { buildHierarchy } from '../layout/buildHierarchy';
import type { useLayout } from '../hooks/useLayout';

export type Hierarchy = ReturnType<typeof buildHierarchy>;
type Layout = ReturnType<typeof useLayout>;

/**
 * Everything a view block might need. App builds this once; each block picks what it uses.
 * Blocks never reach into App internals — this is the whole contract between them.
 */
export interface ViewContext {
  /** Active spec — a drilled-into system, or the platform when not drilled. */
  spec: FdmlDocument;
  /** The top-level platform spec (for blocks that always want the whole platform). */
  platformSpec: FdmlDocument;
  hierarchy: Hierarchy;
  searchQuery: string;
  nodes: Layout['nodes'];
  edges: Layout['edges'];
  onDrillDown: (systemId: string) => void;
}

/**
 * A self-contained view block. Add one (e.g. a C4 block) by creating a file that exports
 * a `ViewBlock` and listing it in `views/index.ts` — nothing else in the app changes.
 */
export interface ViewBlock {
  id: string;
  label: string;
  /** Show the tab only when this returns true (default: always). */
  available?: (ctx: ViewContext) => boolean;
  render: (ctx: ViewContext) => ReactNode;
}
