// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { File } from "./File";
import type { JobReport } from "./JobReport";
import type { LocationResource } from "./LocationResource";
import type { Tag } from "./Tag";

export type CoreResource = { key: "Client" } | { key: "Library" } | { key: "Location", data: LocationResource } | { key: "File", data: File } | { key: "Job", data: JobReport } | { key: "Tag", data: Tag };