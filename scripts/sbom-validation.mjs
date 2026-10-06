import { Spec, Validation } from "@cyclonedx/cyclonedx-library";

const validator = new Validation.JsonValidator(Spec.Version.v1dot5);

export async function validateCycloneDx15(serialized) {
  const error = await validator.validate(serialized);
  if (error) throw new Error(`CycloneDX 1.5 schema validation failed: ${JSON.stringify(error)}`);
  return JSON.parse(serialized);
}
