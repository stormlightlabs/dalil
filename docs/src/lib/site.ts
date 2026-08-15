export const site = {
	name: 'Dalil',
	title: 'Dalil - Evidence-backed repository orientation',
	description: 'Dalil helps people and coding agents orient themselves in unfamiliar codebases.',
	url: 'https://dalil.stormlightlabs.org',
	imagePath: '/og.png',
	imageAlt: 'Dalil documentation.',
	githubUrl: 'https://github.com/stormlightlabs/dalil'
} as const;

export function absoluteUrl(pathname: string): string {
	return new URL(pathname, site.url).toString();
}
