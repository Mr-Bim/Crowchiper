/**
 * Type-safe localStorage wrapper with Valibot validation.
 *
 * Provides a simple API for reading and writing validated data to localStorage.
 */

import { picklist, boolean, safeParse, type InferOutput } from "valibot";

/**
 * Storage key definitions with their schemas and defaults.
 */
const storageSchema = {
  theme: {
    schema: picklist([
      "warm-light",
      "scandi-dark",
      "paper-light",
      "paper-dark",
    ]),
    defaultValue: "scandi-dark" as const,
  },
  "spellcheck-enabled": {
    schema: boolean(),
    defaultValue: false,
  },
} as const;

type StorageKey = keyof typeof storageSchema;
type StorageValue<K extends StorageKey> = InferOutput<
  (typeof storageSchema)[K]["schema"]
>;

/**
 * Get a value from localStorage with type safety and validation.
 * Returns the default value if the key doesn't exist or validation fails.
 */
export function getStorage<K extends StorageKey>(key: K): StorageValue<K> {
  const config = storageSchema[key];
  const stored = localStorage.getItem(key);

  if (stored === null) {
    return config.defaultValue as StorageValue<K>;
  }

  // Parse JSON for non-string types
  let parsed: unknown;
  try {
    // Boolean is stored as "true"/"false" strings
    if (config.schema.type === "boolean") {
      parsed = stored === "true";
    } else {
      // Picklist stores raw string values
      parsed = stored;
    }
  } catch {
    return config.defaultValue as StorageValue<K>;
  }

  // Validate with Valibot
  const result = safeParse(config.schema, parsed);
  if (result.success) {
    return result.output as StorageValue<K>;
  }

  return config.defaultValue as StorageValue<K>;
}

/**
 * Set a value in localStorage with type safety.
 */
export function setStorage<K extends StorageKey>(
  key: K,
  value: StorageValue<K>,
): void {
  // Boolean is stored as "true"/"false" strings
  if (typeof value === "boolean") {
    localStorage.setItem(key, value ? "true" : "false");
  } else {
    localStorage.setItem(key, value as string);
  }
}

/**
 * Remove a value from localStorage.
 */
export function removeStorage(key: StorageKey): void {
  localStorage.removeItem(key);
}
