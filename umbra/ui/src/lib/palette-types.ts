export interface Entry {
	label: string;
	hint: string;
	run: () => void | Promise<void>;
}

export type PaletteMode = '' | 'ask' | 'tabs' | 'history';
