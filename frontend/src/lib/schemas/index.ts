import * as v from 'valibot';

export const AddonMetaSchema = v.object({
  title: v.string(),
  version: v.string(),
  author: v.string(),
  description: v.optional(v.string()),
  dependsOn: v.optional(v.array(v.string())),
  optionalDependsOn: v.optional(v.array(v.string())),
  savedVariables: v.optional(v.array(v.string()))
});

export type AddonMeta = v.InferOutput<typeof AddonMetaSchema>;

export const AppSettingsSchema = v.object({
  addonPath: v.pipe(v.string(), v.trim()),
  autoUpdate: v.boolean(),
  memoryLimitMb: v.number(),
  theme: v.picklist(['scribe', 'neutral', 'dark'])
});

export type AppSettings = v.InferOutput<typeof AppSettingsSchema>;

export const SearchPresetSchema = v.object({
  id: v.string(),
  name: v.pipe(
    v.string(),
    v.trim(),
    v.minLength(1, 'Name is required'),
    v.maxLength(64, 'Name too long')
  ),
  searchQuery: v.pipe(v.string(), v.trim()),
  categoryFilter: v.pipe(v.string(), v.trim()),
  sortBy: v.picklist(['downloads', 'favorites', 'date', 'name']),
  hideInstalled: v.boolean(),
  createdAt: v.string()
});

export type SearchPreset = v.InferOutput<typeof SearchPresetSchema>;

export const SavePresetInputSchema = v.object({
  name: v.pipe(
    v.string(),
    v.trim(),
    v.minLength(1, 'Name is required'),
    v.maxLength(64, 'Name too long')
  )
});

export type SavePresetInput = v.InferOutput<typeof SavePresetInputSchema>;
