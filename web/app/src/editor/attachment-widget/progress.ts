/**
 * Progress tracking types for attachment uploads.
 * Provides granular feedback for each processing stage.
 */

/** Detailed processing stages */
export type UploadStage =
  | "converting" // HEIC to WebP conversion
  | "creating-thumbnails" // Generating sm, md, lg thumbnails
  | "compressing" // Compressing main image
  | "encrypting" // Encrypting all files
  | "uploading"; // Uploading to server

/** Progress state with optional percentage */
export interface UploadProgress {
  stage: UploadStage;
  /** Percentage 0-100, only applicable for uploading stage */
  percent?: number;
}

/** Callback for progress updates */
export type ProgressCallback = (progress: UploadProgress) => void;

/** Human-readable labels for each stage */
export const STAGE_LABELS: Record<UploadStage, string> = {
  converting: "Converting...",
  "creating-thumbnails": "Creating thumbnails...",
  compressing: "Compressing...",
  encrypting: "Encrypting...",
  uploading: "Uploading...",
};

/**
 * Get display text for a progress state.
 * Shows percentage during upload stage.
 */
export function getProgressText(progress: UploadProgress): string {
  if (progress.stage === "uploading" && progress.percent !== undefined) {
    return `Uploading ${progress.percent}%`;
  }
  return STAGE_LABELS[progress.stage];
}
