import { useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Sphere, MeshDistortMaterial } from '@react-three/drei';
import * as THREE from 'three';

const StateWave = ({ position, color, distort }: { position: [number, number, number], color: string, distort: number }) => {
    const meshRef = useRef<THREE.Mesh>(null);
    
    useFrame((state) => {
        if (meshRef.current) {
            meshRef.current.position.y += Math.sin(state.clock.elapsedTime + position[0]) * 0.01;
            meshRef.current.rotation.x += 0.01;
            meshRef.current.rotation.y += 0.01;
        }
    });

    return (
        <Sphere ref={meshRef} args={[0.5, 64, 64]} position={position}>
            <MeshDistortMaterial color={color} attach="material" distort={distort} speed={2} roughness={0.2} metalness={0.8} />
        </Sphere>
    );
};

export const HoloExplorer = () => {
    return (
        <div className="w-full h-[600px] relative glass-panel rounded-3xl overflow-hidden mt-10">
            <div className="absolute top-4 left-4 z-10 text-white">
                <h2 className="text-2xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 to-purple-500">
                    Holographic State Mesh
                </h2>
                <p className="text-sm text-cyan-400 font-mono mt-1">Live Proof-of-Resonance Visualization</p>
            </div>
            
            <Canvas camera={{ position: [0, 0, 5] }}>
                <ambientLight intensity={0.5} />
                <directionalLight position={[10, 10, 5]} intensity={2} color="#06b6d4" />
                <directionalLight position={[-10, -10, -5]} intensity={2} color="#8b5cf6" />
                
                {/* Simulating 3 active state waves seeking resonance */}
                <StateWave position={[-1.5, 0, 0]} color="#06b6d4" distort={0.4} />
                <StateWave position={[0, 0, 0]} color="#8b5cf6" distort={0.6} />
                <StateWave position={[1.5, 0, 0]} color="#10b981" distort={0.4} />

                <OrbitControls enableZoom={false} autoRotate autoRotateSpeed={1.5} />
            </Canvas>
        </div>
    );
};
