// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { LibraryCommand } from './LibraryCommand';

export type ClientCommand =
	| { key: 'CreateLibrary'; params: { name: string } }
	| { key: 'EditLibrary'; params: { id: string; name: string | null; description: string | null } }
	| { key: 'DeleteLibrary'; params: { id: string } }
	| { key: 'LibraryCommand'; params: { library_id: string; command: LibraryCommand } };
