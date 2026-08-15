import { base } from '$app/paths';
import { getDocs } from '$lib/content';

export const prerender = true;

export function GET(): Response {
	const pages = getDocs().map((doc) => `- [${doc.title}](${base}/docs/${doc.slug}.md): ${doc.description}`);
	const body = [
		'# Dalil',
		'',
		'> Evidence-backed repository orientation for people and coding agents.',
		'',
		'Dalil reads a Git repository and its committed history to produce a structural map, an ordered reading plan, and bounded evidence. Use these pages to install Dalil, run a briefing, understand its report formats, and inspect supported manifests.',
		'',
		'## Documentation',
		'',
		...pages,
		''
	].join('\n');

	return new Response(body, { headers: { 'content-type': 'text/plain; charset=utf-8' } });
}
