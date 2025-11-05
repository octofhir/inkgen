
/**
 * Generated TypeScript SDK Package
 * Contains 4 FHIR resources
 */

// Export all resource types
export type { StructureDefinitionResource } from './structuredefinition';
export type { StructureDefinitionResource } from './structuredefinition';
export type { StructureDefinitionResource } from './structuredefinition';
export type { StructureDefinitionResource } from './structuredefinition';


// Export utility types
export interface FhirResource {
  resourceType: string;
  id?: string;
  meta?: {
    versionId?: string;
    lastUpdated?: string;
    profile?: string[];
  };
}

// Resource type union
export type ResourceType = "StructureDefinition" | "StructureDefinition" | "StructureDefinition" | "StructureDefinition";

// Package metadata
export const PACKAGE_INFO = {
  name: "fhir-typescript-sdk",
  version: "1.0.0",
  resourceCount: 4,
  resourceTypes: ["StructureDefinition", "StructureDefinition", "StructureDefinition", "StructureDefinition"],
  generatedAt: new Date().toISOString(),
} as const;
