/**
 * Valibot schemas for API response validation.
 *
 * These schemas ensure type safety at runtime by validating API responses.
 */

import * as v from "valibot";

// --- Posts ---

export const PostSchema = v.object({
  uuid: v.string(),
  title: v.nullable(v.string()),
  title_encrypted: v.boolean(),
  title_iv: v.nullable(v.string()),
  content: v.string(),
  content_encrypted: v.boolean(),
  iv: v.nullable(v.string()),
  encryption_version: v.nullable(v.number()),
  position: v.nullable(v.number()),
  parent_id: v.nullable(v.string()),
  created_at: v.string(),
  updated_at: v.string(),
});

export type Post = v.InferOutput<typeof PostSchema>;

export interface PostNode {
  uuid: string;
  title: string | null;
  title_encrypted: boolean;
  title_iv: string | null;
  content_encrypted: boolean;
  encryption_version: number | null;
  position: number | null;
  parent_id: string | null;
  has_children: boolean;
  children: PostNode[] | null;
  created_at: string;
  updated_at: string;
}

export const PostNodeSchema: v.GenericSchema<PostNode> = v.object({
  uuid: v.string(),
  title: v.nullable(v.string()),
  title_encrypted: v.boolean(),
  title_iv: v.nullable(v.string()),
  content_encrypted: v.boolean(),
  encryption_version: v.nullable(v.number()),
  position: v.nullable(v.number()),
  parent_id: v.nullable(v.string()),
  has_children: v.boolean(),
  children: v.nullable(v.lazy(() => v.array(PostNodeSchema))),
  created_at: v.string(),
  updated_at: v.string(),
}) as v.GenericSchema<PostNode>;

export const DeleteResponseSchema = v.object({
  deleted: v.boolean(),
  children_deleted: v.number(),
});

export type DeleteResponse = v.InferOutput<typeof DeleteResponseSchema>;

// --- Encryption Settings ---

export const UserSettingsSchema = v.object({
  setup_done: v.boolean(),
  encryption_enabled: v.boolean(),
  prf_salt: v.optional(v.string()),
  is_admin: v.boolean(),
  dashboard_path: v.optional(v.string()),
});

export type UserSettings = v.InferOutput<typeof UserSettingsSchema>;

// Keep old name as alias for backward compatibility within encryption-settings.ts
export const EncryptionSettingsSchema = UserSettingsSchema;
export type EncryptionSettings = UserSettings;

export const SetupResponseSchema = v.object({
  prf_salt: v.string(),
});

export type SetupResponse = v.InferOutput<typeof SetupResponseSchema>;

// --- Attachments ---

export const UploadAttachmentResponseSchema = v.object({
  uuid: v.string(),
});

export type UploadAttachmentResponse = v.InferOutput<
  typeof UploadAttachmentResponseSchema
>;

// --- Validation Helper ---

/**
 * Parse and validate data against a schema.
 * Throws a descriptive error if validation fails.
 */
export function validate<T>(
  schema: v.GenericSchema<T>,
  data: unknown,
  context: string,
): T {
  const result = v.safeParse(schema, data);
  if (!result.success) {
    const issues = result.issues
      .map(
        (issue) =>
          `${issue.path?.map((p) => p.key).join(".") || "root"}: ${issue.message}`,
      )
      .join(", ");
    throw new Error(`Invalid ${context}: ${issues}`);
  }
  return result.output;
}
