/**
 * Re-export attachment utilities from shared module.
 * Maintains backward compatibility for imports within the editor chunk.
 */
export {
  parseAttachmentUuids,
  cleanupPendingUploads,
} from "../../shared/attachment-utils.ts";
