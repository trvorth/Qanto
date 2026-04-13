// Qanto Website - Interactive Visualizations

class QantoVisualizations {
    constructor() {
        this.charts = new Map();
        this.animations = new Map();
        this.resizeObserver = null;
        
        this.init();
    }
    
    init() {
        this.setupResizeObserver();
        this.initializeCharts();
        this.createNetworkVisualization();
        this.createPerformanceMetrics();
        this.createConsensusFlow();
        this.createQuantumSecurity();
    }
    
    setupResizeObserver() {
        this.resizeObserver = new ResizeObserver(entries => {
            entries.forEach(entry => {
                const chartId = entry.target.id;
                if (this.charts.has(chartId)) {
                    this.resizeChart(chartId);
                }
            });
        });
    }
    
    initializeCharts() {
        // Initialize all chart containers
        const chartContainers = document.querySelectorAll('[data-chart]');
        chartContainers.forEach(container => {
            const chartType = container.dataset.chart;
            const chartId = container.id;
            
            if (chartId && chartType) {
                this.createChart(chartId, chartType);
                this.resizeObserver.observe(container);
            }
        });
    }
    
    createChart(containerId, chartType) {
        const container = document.getElementById(containerId);
        if (!container) return;
        
        switch (chartType) {
            case 'network':
                this.createNetworkChart(container);
                break;
            case 'performance':
                this.createPerformanceChart(container);
                break;
            case 'consensus':
                this.createConsensusChart(container);
                break;
            case 'security':
                this.createSecurityChart(container);
                break;
            case 'throughput':
                this.createThroughputChart(container);
                break;
            default:
                console.warn(`Unknown chart type: ${chartType}`);
        }
    }
    
    createNetworkVisualization() {
        const container = document.querySelector('.network-visualization');
        if (!container) return;
        
        const svg = this.createSVG(container, 800, 600);
        const width = 800;
        const height = 600;
        
        // Network nodes data
        const nodes = [
            { id: 'genesis', x: width/2, y: height/2, type: 'genesis', connections: [] },
            { id: 'node1', x: width/2 - 150, y: height/2 - 100, type: 'validator', connections: ['genesis'] },
            { id: 'node2', x: width/2 + 150, y: height/2 - 100, type: 'validator', connections: ['genesis'] },
            { id: 'node3', x: width/2 - 150, y: height/2 + 100, type: 'miner', connections: ['genesis', 'node1'] },
            { id: 'node4', x: width/2 + 150, y: height/2 + 100, type: 'miner', connections: ['genesis', 'node2'] },
            { id: 'node5', x: width/2, y: height/2 - 200, type: 'full', connections: ['node1', 'node2'] },
            { id: 'node6', x: width/2, y: height/2 + 200, type: 'full', connections: ['node3', 'node4'] }
        ];
        
        // Create connections
        const connections = [];
        nodes.forEach(node => {
            node.connections.forEach(targetId => {
                const target = nodes.find(n => n.id === targetId);
                if (target) {
                    connections.push({ source: node, target: target });
                }
            });
        });
        
        // Draw connections
        connections.forEach((conn, index) => {
            const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
            line.setAttribute('x1', conn.source.x);
            line.setAttribute('y1', conn.source.y);
            line.setAttribute('x2', conn.target.x);
            line.setAttribute('y2', conn.target.y);
            line.setAttribute('stroke', 'rgba(102, 126, 234, 0.3)');
            line.setAttribute('stroke-width', '2');
            line.style.strokeDasharray = '5,5';
            line.style.animation = `pulse-line 2s ease-in-out infinite ${index * 0.2}s`;
            
            svg.appendChild(line);
        });
        
        // Draw nodes
        nodes.forEach((node, index) => {
            const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
            group.setAttribute('transform', `translate(${node.x}, ${node.y})`);
            
            const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
            circle.setAttribute('r', node.type === 'genesis' ? '20' : '15');
            circle.setAttribute('fill', this.getNodeColor(node.type));
            circle.setAttribute('stroke', '#ffffff');
            circle.setAttribute('stroke-width', '3');
            circle.style.animation = `node-pulse 3s ease-in-out infinite ${index * 0.5}s`;
            
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('text-anchor', 'middle');
            text.setAttribute('dy', '35');
            text.setAttribute('fill', '#333');
            text.setAttribute('font-size', '12');
            text.setAttribute('font-family', 'Inter, sans-serif');
            text.textContent = node.id;
            
            group.appendChild(circle);
            group.appendChild(text);
            svg.appendChild(group);
        });
        
        this.addNetworkAnimationStyles();
    }
    
    createPerformanceMetrics() {
        const container = document.querySelector('.performance-metrics');
        if (!container) return;
        
        const metrics = [
            { label: 'Transactions/sec', value: 100000, max: 150000, color: '#667eea' },
            { label: 'Block Time', value: 2, max: 10, color: '#764ba2', unit: 's' },
            { label: 'Finality Time', value: 500, max: 2000, color: '#f093fb', unit: 'ms' },
            { label: 'Network Nodes', value: 1247, max: 2000, color: '#4facfe' }
        ];
        
        const svg = this.createSVG(container, 600, 400);
        const barWidth = 120;
        const barHeight = 200;
        const spacing = 140;
        const startX = 50;
        const startY = 300;
        
        metrics.forEach((metric, index) => {
            const x = startX + index * spacing;
            const percentage = metric.value / metric.max;
            const height = barHeight * percentage;
            
            // Background bar
            const bgRect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
            bgRect.setAttribute('x', x);
            bgRect.setAttribute('y', startY - barHeight);
            bgRect.setAttribute('width', barWidth);
            bgRect.setAttribute('height', barHeight);
            bgRect.setAttribute('fill', 'rgba(0, 0, 0, 0.1)');
            bgRect.setAttribute('rx', '10');
            
            // Value bar
            const valueRect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
            valueRect.setAttribute('x', x);
            valueRect.setAttribute('y', startY - height);
            valueRect.setAttribute('width', barWidth);
            valueRect.setAttribute('height', height);
            valueRect.setAttribute('fill', metric.color);
            valueRect.setAttribute('rx', '10');
            valueRect.style.animation = `bar-grow 2s ease-out ${index * 0.3}s both`;
            
            // Label
            const label = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            label.setAttribute('x', x + barWidth / 2);
            label.setAttribute('y', startY + 20);
            label.setAttribute('text-anchor', 'middle');
            label.setAttribute('fill', '#333');
            label.setAttribute('font-size', '12');
            label.setAttribute('font-family', 'Inter, sans-serif');
            label.textContent = metric.label;
            
            // Value
            const value = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            value.setAttribute('x', x + barWidth / 2);
            value.setAttribute('y', startY + 35);
            value.setAttribute('text-anchor', 'middle');
            value.setAttribute('fill', metric.color);
            value.setAttribute('font-size', '14');
            value.setAttribute('font-weight', 'bold');
            value.setAttribute('font-family', 'Inter, sans-serif');
            value.textContent = `${metric.value.toLocaleString()}${metric.unit || ''}`;
            
            svg.appendChild(bgRect);
            svg.appendChild(valueRect);
            svg.appendChild(label);
            svg.appendChild(value);
        });
        
        this.addPerformanceAnimationStyles();
    }
    
    createConsensusFlow() {
        const container = document.querySelector('.consensus-flow');
        if (!container) return;
        
        const svg = this.createSVG(container, 700, 300);
        const steps = [
            { name: 'PoW Mining', x: 100, y: 150, color: '#667eea' },
            { name: 'DPoS Validation', x: 300, y: 150, color: '#764ba2' },
            { name: 'PoSe Finalization', x: 500, y: 150, color: '#f093fb' },
            { name: 'Block Confirmed', x: 650, y: 150, color: '#4facfe' }
        ];
        
        // Draw flow arrows
        for (let i = 0; i < steps.length - 1; i++) {
            const arrow = this.createArrow(
                steps[i].x + 40,
                steps[i].y,
                steps[i + 1].x - 40,
                steps[i + 1].y
            );
            arrow.style.animation = `arrow-flow 3s ease-in-out infinite ${i * 0.5}s`;
            svg.appendChild(arrow);
        }
        
        // Draw steps
        steps.forEach((step, index) => {
            const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
            group.setAttribute('transform', `translate(${step.x}, ${step.y})`);
            
            const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
            circle.setAttribute('r', '30');
            circle.setAttribute('fill', step.color);
            circle.setAttribute('stroke', '#ffffff');
            circle.setAttribute('stroke-width', '3');
            circle.style.animation = `step-pulse 2s ease-in-out infinite ${index * 0.4}s`;
            
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('text-anchor', 'middle');
            text.setAttribute('dy', '5');
            text.setAttribute('fill', '#ffffff');
            text.setAttribute('font-size', '10');
            text.setAttribute('font-weight', 'bold');
            text.setAttribute('font-family', 'Inter, sans-serif');
            text.textContent = (index + 1).toString();
            
            const label = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            label.setAttribute('text-anchor', 'middle');
            label.setAttribute('dy', '50');
            label.setAttribute('fill', '#333');
            label.setAttribute('font-size', '12');
            label.setAttribute('font-family', 'Inter, sans-serif');
            label.textContent = step.name;
            
            group.appendChild(circle);
            group.appendChild(text);
            group.appendChild(label);
            svg.appendChild(group);
        });
        
        this.addConsensusAnimationStyles();
    }
    
    createQuantumSecurity() {
        const container = document.querySelector('.quantum-security');
        if (!container) return;
        
        const svg = this.createSVG(container, 500, 400);
        const centerX = 250;
        const centerY = 200;
        const radius = 120;
        
        const algorithms = [
            { name: 'CRYSTALS-Dilithium', angle: 0, color: '#667eea' },
            { name: 'Kyber', angle: Math.PI / 2, color: '#764ba2' },
            { name: 'SPHINCS+', angle: Math.PI, color: '#f093fb' },
            { name: 'Homomorphic', angle: 3 * Math.PI / 2, color: '#4facfe' }
        ];
        
        // Central quantum core
        const core = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
        core.setAttribute('cx', centerX);
        core.setAttribute('cy', centerY);
        core.setAttribute('r', '40');
        core.setAttribute('fill', 'url(#quantumGradient)');
        core.setAttribute('stroke', '#ffffff');
        core.setAttribute('stroke-width', '3');
        core.style.animation = 'quantum-pulse 3s ease-in-out infinite';
        
        // Gradient definition
        const defs = document.createElementNS('http://www.w3.org/2000/svg', 'defs');
        const gradient = document.createElementNS('http://www.w3.org/2000/svg', 'radialGradient');
        gradient.setAttribute('id', 'quantumGradient');
        
        const stop1 = document.createElementNS('http://www.w3.org/2000/svg', 'stop');
        stop1.setAttribute('offset', '0%');
        stop1.setAttribute('style', 'stop-color:#667eea;stop-opacity:1');
        
        const stop2 = document.createElementNS('http://www.w3.org/2000/svg', 'stop');
        stop2.setAttribute('offset', '100%');
        stop2.setAttribute('style', 'stop-color:#764ba2;stop-opacity:1');
        
        gradient.appendChild(stop1);
        gradient.appendChild(stop2);
        defs.appendChild(gradient);
        svg.appendChild(defs);
        
        // Algorithm nodes
        algorithms.forEach((algo, index) => {
            const x = centerX + Math.cos(algo.angle) * radius;
            const y = centerY + Math.sin(algo.angle) * radius;
            
            // Connection line
            const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
            line.setAttribute('x1', centerX);
            line.setAttribute('y1', centerY);
            line.setAttribute('x2', x);
            line.setAttribute('y2', y);
            line.setAttribute('stroke', algo.color);
            line.setAttribute('stroke-width', '2');
            line.setAttribute('opacity', '0.6');
            line.style.animation = `connection-pulse 2s ease-in-out infinite ${index * 0.5}s`;
            
            // Algorithm node
            const node = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
            node.setAttribute('cx', x);
            node.setAttribute('cy', y);
            node.setAttribute('r', '25');
            node.setAttribute('fill', algo.color);
            node.setAttribute('stroke', '#ffffff');
            node.setAttribute('stroke-width', '2');
            node.style.animation = `algo-orbit 8s linear infinite ${index * 2}s`;
            
            // Algorithm label
            const label = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            label.setAttribute('x', x);
            label.setAttribute('y', y + 45);
            label.setAttribute('text-anchor', 'middle');
            label.setAttribute('fill', '#333');
            label.setAttribute('font-size', '10');
            label.setAttribute('font-family', 'Inter, sans-serif');
            label.textContent = algo.name;
            
            svg.appendChild(line);
            svg.appendChild(node);
            svg.appendChild(label);
        });
        
        svg.appendChild(core);
        
        // Core label
        const coreLabel = document.createElementNS('http://www.w3.org/2000/svg', 'text');
        coreLabel.setAttribute('x', centerX);
        coreLabel.setAttribute('y', centerY + 5);
        coreLabel.setAttribute('text-anchor', 'middle');
        coreLabel.setAttribute('fill', '#ffffff');
        coreLabel.setAttribute('font-size', '12');
        coreLabel.setAttribute('font-weight', 'bold');
        coreLabel.setAttribute('font-family', 'Inter, sans-serif');
        coreLabel.textContent = 'Quantum';
        
        svg.appendChild(coreLabel);
        
        this.addQuantumAnimationStyles();
    }
    
    // Utility methods
    createSVG(container, width, height) {
        const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        svg.setAttribute('width', '100%');
        svg.setAttribute('height', '100%');
        svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
        svg.style.maxWidth = width + 'px';
        svg.style.maxHeight = height + 'px';
        
        container.innerHTML = '';
        container.appendChild(svg);
        
        return svg;
    }
    
    createArrow(x1, y1, x2, y2) {
        const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        
        // Arrow line
        const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
        line.setAttribute('x1', x1);
        line.setAttribute('y1', y1);
        line.setAttribute('x2', x2);
        line.setAttribute('y2', y2);
        line.setAttribute('stroke', '#667eea');
        line.setAttribute('stroke-width', '2');
        line.setAttribute('marker-end', 'url(#arrowhead)');
        
        // Arrow marker
        if (!document.getElementById('arrowhead')) {
            const defs = document.createElementNS('http://www.w3.org/2000/svg', 'defs');
            const marker = document.createElementNS('http://www.w3.org/2000/svg', 'marker');
            marker.setAttribute('id', 'arrowhead');
            marker.setAttribute('markerWidth', '10');
            marker.setAttribute('markerHeight', '7');
            marker.setAttribute('refX', '9');
            marker.setAttribute('refY', '3.5');
            marker.setAttribute('orient', 'auto');
            
            const polygon = document.createElementNS('http://www.w3.org/2000/svg', 'polygon');
            polygon.setAttribute('points', '0 0, 10 3.5, 0 7');
            polygon.setAttribute('fill', '#667eea');
            
            marker.appendChild(polygon);
            defs.appendChild(marker);
            group.appendChild(defs);
        }
        
        group.appendChild(line);
        return group;
    }
    
    getNodeColor(type) {
        const colors = {
            genesis: '#667eea',
            validator: '#764ba2',
            miner: '#f093fb',
            full: '#4facfe'
        };
        return colors[type] || '#cccccc';
    }
    
    resizeChart(chartId) {
        // Implement chart resizing logic
        const container = document.getElementById(chartId);
        if (container) {
            const chartType = container.dataset.chart;
            this.createChart(chartId, chartType);
        }
    }
    
    // Animation styles
    addNetworkAnimationStyles() {
        if (document.getElementById('network-animations')) return;
        
        const style = document.createElement('style');
        style.id = 'network-animations';
        style.textContent = `
            @keyframes pulse-line {
                0%, 100% { stroke-opacity: 0.3; }
                50% { stroke-opacity: 1; }
            }
            @keyframes node-pulse {
                0%, 100% { transform: scale(1); }
                50% { transform: scale(1.1); }
            }
        `;
        document.head.appendChild(style);
    }
    
    addPerformanceAnimationStyles() {
        if (document.getElementById('performance-animations')) return;
        
        const style = document.createElement('style');
        style.id = 'performance-animations';
        style.textContent = `
            @keyframes bar-grow {
                0% { height: 0; }
                100% { height: attr(height); }
            }
        `;
        document.head.appendChild(style);
    }
    
    addConsensusAnimationStyles() {
        if (document.getElementById('consensus-animations')) return;
        
        const style = document.createElement('style');
        style.id = 'consensus-animations';
        style.textContent = `
            @keyframes arrow-flow {
                0% { opacity: 0.3; }
                50% { opacity: 1; }
                100% { opacity: 0.3; }
            }
            @keyframes step-pulse {
                0%, 100% { transform: scale(1); }
                50% { transform: scale(1.05); }
            }
        `;
        document.head.appendChild(style);
    }
    
    addQuantumAnimationStyles() {
        if (document.getElementById('quantum-animations')) return;
        
        const style = document.createElement('style');
        style.id = 'quantum-animations';
        style.textContent = `
            @keyframes quantum-pulse {
                0%, 100% { transform: scale(1); opacity: 1; }
                50% { transform: scale(1.1); opacity: 0.8; }
            }
            @keyframes connection-pulse {
                0%, 100% { stroke-opacity: 0.3; }
                50% { stroke-opacity: 1; }
            }
            @keyframes algo-orbit {
                0% { transform: rotate(0deg); }
                100% { transform: rotate(360deg); }
            }
        `;
        document.head.appendChild(style);
    }
    
    // Cleanup
    destroy() {
        if (this.resizeObserver) {
            this.resizeObserver.disconnect();
        }
        
        this.charts.clear();
        this.animations.clear();
    }
}

// Initialize visualizations when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.qantoVisualizations = new QantoVisualizations();
});

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = QantoVisualizations;
}