<script lang="ts">
	import { META } from '$lib/constants';

	interface Props {
		width: number;
		height: number;
		faviconDataUrl: string;
	}

	let { width, height, faviconDataUrl }: Props = $props();
</script>

<div
	style="display:flex; position:relative; width:{width}px; height:{height}px; overflow:hidden; background:#141415; color:#f7f1f3; font-family:'IBM Plex Sans';">
	<div
		style="display:flex; position:absolute; inset:0; background:linear-gradient(125deg, #141415 0%, #262124 64%, #3b2027 100%);">
	</div>
	<div
		style="display:flex; position:absolute; top:-190px; right:-105px; width:550px; height:550px; border:1px solid #66585d; border-radius:275px; opacity:0.72;">
	</div>
	<div
		style="display:flex; position:absolute; top:-120px; right:-35px; width:410px; height:410px; border:1px solid #66585d; border-radius:205px; opacity:0.66;">
	</div>
	<div
		style="display:flex; position:absolute; right:106px; top:82px; width:12px; height:12px; border-radius:6px; background:#ef596f;">
	</div>
	<div
		style="display:flex; position:absolute; right:159px; top:121px; width:7px; height:7px; border-radius:4px; background:#ff8da0; opacity:0.75;">
	</div>

	<div style="display:flex; position:relative; width:100%; flex-direction:column; padding:48px 58px 50px;">
		<div
			style="display:flex; align-items:center; gap:13px; color:#ffb1bd; font-family:'Google Sans'; font-size:23px; font-weight:700; letter-spacing:-0.4px;">
			<img src={faviconDataUrl} alt="" style="display:flex; width:31px; height:31px;" />
			<span>Dalil</span>
		</div>

		<div style="display:flex; flex: 1; margin-top:46px; flex-direction:column;">
			<div
				style="display:flex; color:#ff8da0; font-family:'Google Sans'; font-size:18px; font-weight:600; letter-spacing:2.2px;">
				{META.title}
			</div>
			<div
				style="display:flex; max-width:760px; margin-top:10px; color:#ffffff; font-family:'Google Sans'; font-size:72px; font-weight:700; letter-spacing:-2.8px; line-height:0.98;">
				Find your way through unfamiliar code.
			</div>
			<div style="display:flex; max-width:610px; margin-top:20px; color:#d8cdd1; font-size:25px; line-height:1.32;">
				{META.tagline}
			</div>
		</div>

		<div
			style="display:flex; align-items:center; gap:10px; color:#d8cdd1; font-family:'IBM Plex Sans'; font-size:19px; line-height:1;">
			<div style="display:flex; width:8px; height:8px; border-radius:4px; background:#ef596f;"></div>
			<span>github.com/stormlightlabs/dalil</span>
		</div>
	</div>
</div>
