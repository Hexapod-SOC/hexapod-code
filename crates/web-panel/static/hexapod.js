import * as THREE from 'https://esm.sh/three@0.160.0';
import { OrbitControls } from 'https://esm.sh/three@0.160.0/examples/jsm/controls/OrbitControls.js';

const container = document.getElementById('hexapod-visualizer');
if (!container) {
	console.warn('Hexapod visualizer container not found');
} else {
	const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
	renderer.setPixelRatio(window.devicePixelRatio || 1);
	renderer.setSize(container.clientWidth, container.clientHeight);
	renderer.setClearColor(0x000000, 0);
	container.appendChild(renderer.domElement);

	const scene = new THREE.Scene();

	const camera = new THREE.PerspectiveCamera(
		45,
		container.clientWidth / container.clientHeight,
		0.1,
		2000
	);
	camera.position.set(240, 180, 240);

	const controls = new OrbitControls(camera, renderer.domElement);
	controls.enableDamping = true;
	controls.dampingFactor = 0.08;
	controls.target.set(0, 40, 0);
	controls.update();

	const ambient = new THREE.AmbientLight(0xffffff, 0.6);
	scene.add(ambient);
	const keyLight = new THREE.DirectionalLight(0xffffff, 0.8);
	keyLight.position.set(200, 300, 100);
	scene.add(keyLight);
	const rimLight = new THREE.DirectionalLight(0x88aaff, 0.4);
	rimLight.position.set(-200, 200, -150);
	scene.add(rimLight);

	const grid = new THREE.GridHelper(400, 20, 0x334466, 0x223344);
	grid.position.y = -60;
	scene.add(grid);

	const hexapod = new THREE.Group();
	scene.add(hexapod);

	const bodyMaterial = new THREE.MeshStandardMaterial({
		color: 0x667eea,
		metalness: 0.4,
		roughness: 0.35
	});
	const bodyGeometry = new THREE.BoxGeometry(140, 20, 90);
	const bodyMesh = new THREE.Mesh(bodyGeometry, bodyMaterial);
	bodyMesh.position.y = 40;
	hexapod.add(bodyMesh);

	const eyeMaterial = new THREE.MeshStandardMaterial({
		color: 0x36f5ff,
		emissive: 0x36f5ff,
		emissiveIntensity: 0.8,
		metalness: 0.1,
		roughness: 0.2
	});
	const eyeGeometry = new THREE.SphereGeometry(4, 16, 16);
	const leftEye = new THREE.Mesh(eyeGeometry, eyeMaterial);
	const rightEye = new THREE.Mesh(eyeGeometry, eyeMaterial);
	const eyeOffsetX = 70;
	const eyeOffsetY = 42;
	const eyeOffsetZ = 12;
	leftEye.position.set(eyeOffsetX, eyeOffsetY, -eyeOffsetZ);
	rightEye.position.set(eyeOffsetX, eyeOffsetY, eyeOffsetZ);
	hexapod.add(leftEye, rightEye);

	const legMaterial = new THREE.MeshStandardMaterial({
		color: 0xeeeeee,
		metalness: 0.2,
		roughness: 0.6
	});

	const coxaLength = 28;
	const femurLength = 55;
	const tibiaLength = 70;

	const legConfigs = [
		{ id: 'lf', name: 'left_front', x: 55, z: -40, side: -1, phase: 0.0 },
		{ id: 'lm', name: 'left_middle', x: 0, z: -48, side: -1, phase: 0.5 },
		{ id: 'lb', name: 'left_back', x: -55, z: -40, side: -1, phase: 0.0 },
		{ id: 'rf', name: 'right_front', x: 55, z: 40, side: 1, phase: 0.5 },
		{ id: 'rm', name: 'right_middle', x: 0, z: 48, side: 1, phase: 0.0 },
		{ id: 'rb', name: 'right_back', x: -55, z: 40, side: 1, phase: 0.5 }
	];

	function createLeg(config) {
		const legRoot = new THREE.Group();
		legRoot.position.set(config.x, 40, config.z);

		const coxaPivot = new THREE.Group();
		legRoot.add(coxaPivot);

		const coxaGeom = new THREE.BoxGeometry(10, 10, coxaLength);
		const coxaMesh = new THREE.Mesh(coxaGeom, legMaterial);
		coxaMesh.position.z = (coxaLength / 2) * config.side;
		coxaPivot.add(coxaMesh);

		const femurPivot = new THREE.Group();
		femurPivot.position.z = coxaLength * config.side;
		coxaPivot.add(femurPivot);

		const femurGeom = new THREE.BoxGeometry(10, femurLength, 10);
		const femurMesh = new THREE.Mesh(femurGeom, legMaterial);
		femurMesh.position.y = -femurLength / 2;
		femurPivot.add(femurMesh);

		const tibiaPivot = new THREE.Group();
		tibiaPivot.position.y = -femurLength;
		femurPivot.add(tibiaPivot);

		const tibiaGeom = new THREE.BoxGeometry(8, tibiaLength, 8);
		const tibiaMesh = new THREE.Mesh(tibiaGeom, legMaterial);
		tibiaMesh.position.y = -tibiaLength / 2;
		tibiaPivot.add(tibiaMesh);

		return { config, legRoot, coxaPivot, femurPivot, tibiaPivot };
	}

	const legs = legConfigs.map((cfg) => {
		const leg = createLeg(cfg);
		hexapod.add(leg.legRoot);
		return leg;
	});

	let pose = { roll: 0, pitch: 0, yaw: 0 };
	let movement = { forward: 0, strafe: 0, rotation: 0 };
	let gaitName = '--';
	let lastStatusAt = 0;
	let kinematics = null;
	let lastFrameTime = performance.now();
	let fetchInFlight = false;
	const legAngles = new Map();
	const OFFSET = {
		left: {
			coxa: Math.PI / 4,
			femur: Math.PI / 4,
			tibia: -Math.PI
		},
		right: {
			coxa: Math.PI / 2,
			femur: Math.PI - Math.PI / 4,
			tibia: -Math.PI + Math.PI / 4
		}
	};

	const API_BASE = `${window.location.protocol}//${window.location.hostname}:3000/api`;

	const gaitBadge = document.getElementById('viz-gait');
	const speedBadge = document.getElementById('viz-speed');
	const resetBtn = document.getElementById('viz-reset');

	if (resetBtn) {
		resetBtn.addEventListener('click', () => {
			camera.position.set(240, 180, 240);
			controls.target.set(0, 40, 0);
			controls.update();
		});
	}

	window.addEventListener('hexapod:pose', (event) => {
		if (!event.detail) return;
		pose = {
			roll: event.detail.roll || 0,
			pitch: event.detail.pitch || 0,
			yaw: event.detail.yaw || 0
		};
	});

	window.addEventListener('hexapod:move', (event) => {
		if (!event.detail) return;
		movement = {
			forward: event.detail.forward || 0,
			strafe: event.detail.strafe || 0,
			rotation: event.detail.rotation || 0
		};    
	});

	window.addEventListener('hexapod:status', (event) => {
		if (!event.detail) return;
		gaitName = event.detail.gait_name || gaitName;
		lastStatusAt = performance.now();
	});

	async function pollKinematics() {
		if (fetchInFlight) return;
		fetchInFlight = true;
		try {
			const controller = new AbortController();
			const timeout = setTimeout(() => controller.abort(), 500);
			const res = await fetch(`${API_BASE}/legs`, { signal: controller.signal });
			clearTimeout(timeout);
			if (!res.ok) return;
			const data = await res.json();
			kinematics = data;
			if (data.gait_name) gaitName = data.gait_name;
			if (data.body_pose) {
				pose = {
					roll: data.body_pose.roll || 0,
					pitch: data.body_pose.pitch || 0,
					yaw: data.body_pose.yaw || 0
				};
			}
			if (data.velocity) {
				movement = {
					forward: data.velocity[0] || 0,
					strafe: data.velocity[1] || 0,
					rotation: data.rotation || 0
				};
			}
			lastStatusAt = performance.now();
		} catch (_) {
			// ignore
		} finally {
			fetchInFlight = false;
		}
	}

	(async function kinematicsLoop() {
		while (true) {
			await pollKinematics();
			await new Promise((resolve) => setTimeout(resolve, 100));
		}
	})();

	function updateBadges(speed) {
		if (gaitBadge) gaitBadge.textContent = `Gait: ${gaitName}`;
		if (speedBadge) speedBadge.textContent = `Speed: ${speed.toFixed(0)} mm/s`;
	}

	function animate(time) {
		requestAnimationFrame(animate);
		const dt = Math.min(0.05, (time - lastFrameTime) / 1000);
		lastFrameTime = time;

		const speed = Math.min(120, Math.hypot(movement.forward, movement.strafe));

		const rollRad = THREE.MathUtils.degToRad(pose.roll || 0);
		const pitchRad = THREE.MathUtils.degToRad(pose.pitch || 0);
		const yawRad = THREE.MathUtils.degToRad(pose.yaw || 0);

		hexapod.rotation.set(pitchRad, yawRad, rollRad);

		if (kinematics && kinematics.legs) {
			const data = kinematics.legs;
			const lookup = {
				left_front: data.left_front,
				left_middle: data.left_middle,
				left_back: data.left_back,
				right_front: data.right_front,
				right_middle: data.right_middle,
				right_back: data.right_back
			};

			legs.forEach((leg) => {
				const entry = lookup[leg.config.name];
				if (!entry || !entry.angles_rad) return;

				const target = legAngles.get(leg.config.name) || {
					coxa: 0,
					femur: 0,
					tibia: 0
				};

				const [coxa, femur, tibia] = entry.angles_rad;
				const sideOffsets = leg.config.side === -1 ? OFFSET.left : OFFSET.right;
				const coxaSign = leg.config.side === -1 ? -1 : 1;
				const femurSign = leg.config.side === -1 ? -1 : 1;
				const tibiaSign = leg.config.side === -1 ? -1 : 1;
				target.coxa = coxaSign * coxa + sideOffsets.coxa;
				target.femur = femurSign * femur + sideOffsets.femur;
				target.tibia = tibiaSign * tibia + sideOffsets.tibia;
				legAngles.set(leg.config.name, target);
			});
		}

		const lerpFactor = 1.0 - Math.exp(-dt * 12.0);
		legs.forEach((leg) => {
			const target = legAngles.get(leg.config.name);
			if (!target) return;

			leg.coxaPivot.rotation.y = THREE.MathUtils.lerp(
				leg.coxaPivot.rotation.y,
				target.coxa,
				lerpFactor
			);
			leg.femurPivot.rotation.x = THREE.MathUtils.lerp(
				leg.femurPivot.rotation.x,
				target.femur,
				lerpFactor
			);
			leg.tibiaPivot.rotation.x = THREE.MathUtils.lerp(
				leg.tibiaPivot.rotation.x,
				target.tibia,
				lerpFactor
			);
		});

		updateBadges(speed);
		controls.update();
		renderer.render(scene, camera);
	}

	const resizeObserver = new ResizeObserver(() => {
		const { clientWidth, clientHeight } = container;
		if (clientWidth === 0 || clientHeight === 0) return;
		camera.aspect = clientWidth / clientHeight;
		camera.updateProjectionMatrix();
		renderer.setSize(clientWidth, clientHeight);
	});
	resizeObserver.observe(container);

	animate();
}