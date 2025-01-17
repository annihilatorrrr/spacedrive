import { httpLink, initRspc, type AlphaClient } from '@oscartbeaumont-sd/rspc-client/v2';
import { useEffect, useRef, useState } from 'react';
import { useDiscoveredPeers, type Procedures } from '@sd/client';
import { Button } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

export const Component = () => {
	// TODO: Handle if P2P is disabled
	const [activePeer, setActivePeer] = useState<string | null>(null);

	return (
		<div className="p-4">
			{activePeer ? (
				<P2PInfo peer={activePeer} />
			) : (
				<PeerSelector setActivePeer={setActivePeer} />
			)}
		</div>
	);
};

function PeerSelector({ setActivePeer }: { setActivePeer: (peer: string) => void }) {
	const peers = useDiscoveredPeers();

	return (
		<>
			<h1>Nodes:</h1>
			{peers.size === 0 ? (
				<p>No peers found...</p>
			) : (
				<ul>
					{[...peers.entries()].map(([id, _node]) => (
						<li key={id}>
							{id}
							<Button onClick={() => setActivePeer(id)}>Connect</Button>
						</li>
					))}
				</ul>
			)}
		</>
	);
}

function P2PInfo({ peer }: { peer: string }) {
	const platform = usePlatform();
	const ref = useRef<AlphaClient<Procedures>>();
	const [result, setResult] = useState('');
	useEffect(() => {
		// TODO: Cleanup when URL changed
		const endpoint = platform.getRemoteRspcEndpoint(peer);
		ref.current = initRspc<Procedures>({
			links: [
				httpLink({
					url: endpoint.url,
					headers: endpoint.headers
				})
			]
		});
	}, [peer]);

	useEffect(() => {
		if (!ref.current) return;
		ref.current.query(['nodeState']).then((data) => setResult(JSON.stringify(data, null, 2)));
	}, [ref, result]);

	return (
		<div className="flex flex-col">
			<h1>Connected with: {peer}</h1>

			<Button
				onClick={() => {
					ref.current
						?.query(['nodeState'])
						.then((data) => setResult(JSON.stringify(data, null, 2)));
				}}
			>
				Refetch
			</Button>
			<pre>{result}</pre>
		</div>
	);
}
