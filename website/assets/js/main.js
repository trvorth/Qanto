// Qanto Main Website - Core JavaScript

class QantoWebsite {
    constructor() {
        this.isLoaded = false;
        this.scrollPosition = 0;
        this.isScrolling = false;
        this.activeSection = 'home';
        this.mobileMenuOpen = false;
        
        this.init();
    }
    
    init() {
        this.bindEvents();
        this.initializeComponents();
        this.handleLoading();
        this.setupIntersectionObserver();
        this.initializeAnimations();
    }
    
    bindEvents() {
        // Window events
        window.addEventListener('load', () => this.handlePageLoad());
        window.addEventListener('scroll', () => this.handleScroll());
        window.addEventListener('resize', () => this.handleResize());
        
        // Navigation events
        document.addEventListener('click', (e) => this.handleNavigation(e));
        
        // Mobile menu toggle
        const navToggle = document.getElementById('nav-toggle');
        if (navToggle) {
            navToggle.addEventListener('click', () => this.toggleMobileMenu());
        }
        
        // Back to top button
        const backToTop = document.getElementById('back-to-top');
        if (backToTop) {
            backToTop.addEventListener('click', () => this.scrollToTop());
        }
        
        // Technology tabs
        const techTabs = document.querySelectorAll('.tech-tab');
        techTabs.forEach(tab => {
            tab.addEventListener('click', (e) => this.switchTechTab(e));
        });
        
        // Form submissions
        const forms = document.querySelectorAll('form');
        forms.forEach(form => {
            form.addEventListener('submit', (e) => this.handleFormSubmit(e));
        });
        
        // Keyboard navigation
        document.addEventListener('keydown', (e) => this.handleKeyboard(e));
    }
    
    initializeComponents() {
        this.initializeCounters();
        this.initializeParticles();
        this.initializeDAGVisualization();
        this.initializeTechVisualizations();
        this.initializeCodeHighlighting();
    }
    
    handleLoading() {
        const loadingScreen = document.getElementById('loading-screen');
        if (loadingScreen) {
            // Simulate loading time
            setTimeout(() => {
                loadingScreen.classList.add('hidden');
                setTimeout(() => {
                    loadingScreen.style.display = 'none';
                }, 500);
            }, 1500);
        }
    }
    
    handlePageLoad() {
        this.isLoaded = true;
        this.updateActiveNavLink();
        this.animateCounters();
        
        // Initialize AOS (Animate On Scroll) if available
        if (typeof AOS !== 'undefined') {
            AOS.init({
                duration: 800,
                easing: 'ease-out-cubic',
                once: true,
                offset: 100
            });
        }
    }
    
    handleScroll() {
        if (this.isScrolling) return;
        
        this.isScrolling = true;
        requestAnimationFrame(() => {
            this.scrollPosition = window.pageYOffset;
            
            this.updateNavbarState();
            this.updateBackToTopButton();
            this.updateActiveSection();
            this.handleParallaxEffects();
            
            this.isScrolling = false;
        });
    }
    
    handleResize() {
        // Close mobile menu on resize
        if (window.innerWidth > 768 && this.mobileMenuOpen) {
            this.toggleMobileMenu();
        }
        
        // Recalculate visualizations
        this.resizeVisualizations();
    }
    
    handleNavigation(e) {
        const link = e.target.closest('a[href^="#"]');
        if (!link) return;
        
        e.preventDefault();
        const targetId = link.getAttribute('href').substring(1);
        const targetElement = document.getElementById(targetId);
        
        if (targetElement) {
            this.smoothScrollTo(targetElement);
            
            // Close mobile menu if open
            if (this.mobileMenuOpen) {
                this.toggleMobileMenu();
            }
        }
    }
    
    handleKeyboard(e) {
        // Escape key closes mobile menu
        if (e.key === 'Escape' && this.mobileMenuOpen) {
            this.toggleMobileMenu();
        }
        
        // Arrow keys for tech tabs
        if (e.target.classList.contains('tech-tab')) {
            const tabs = Array.from(document.querySelectorAll('.tech-tab'));
            const currentIndex = tabs.indexOf(e.target);
            
            if (e.key === 'ArrowLeft' && currentIndex > 0) {
                tabs[currentIndex - 1].click();
                tabs[currentIndex - 1].focus();
            } else if (e.key === 'ArrowRight' && currentIndex < tabs.length - 1) {
                tabs[currentIndex + 1].click();
                tabs[currentIndex + 1].focus();
            }
        }
    }
    
    toggleMobileMenu() {
        const navMenu = document.getElementById('nav-menu');
        const navToggle = document.getElementById('nav-toggle');
        
        if (!navMenu || !navToggle) return;
        
        this.mobileMenuOpen = !this.mobileMenuOpen;
        
        navMenu.classList.toggle('active', this.mobileMenuOpen);
        navToggle.classList.toggle('active', this.mobileMenuOpen);
        
        // Prevent body scroll when menu is open
        document.body.style.overflow = this.mobileMenuOpen ? 'hidden' : '';
    }
    
    updateNavbarState() {
        const navbar = document.getElementById('navbar');
        if (!navbar) return;
        
        if (this.scrollPosition > 100) {
            navbar.classList.add('scrolled');
        } else {
            navbar.classList.remove('scrolled');
        }
    }
    
    updateBackToTopButton() {
        const backToTop = document.getElementById('back-to-top');
        if (!backToTop) return;
        
        if (this.scrollPosition > 500) {
            backToTop.classList.add('visible');
        } else {
            backToTop.classList.remove('visible');
        }
    }
    
    updateActiveSection() {
        const sections = document.querySelectorAll('section[id]');
        let currentSection = 'home';
        
        sections.forEach(section => {
            const rect = section.getBoundingClientRect();
            if (rect.top <= 100 && rect.bottom >= 100) {
                currentSection = section.id;
            }
        });
        
        if (currentSection !== this.activeSection) {
            this.activeSection = currentSection;
            this.updateActiveNavLink();
        }
    }
    
    updateActiveNavLink() {
        const navLinks = document.querySelectorAll('.nav-link');
        navLinks.forEach(link => {
            const href = link.getAttribute('href');
            if (href === `#${this.activeSection}`) {
                link.classList.add('active');
            } else {
                link.classList.remove('active');
            }
        });
    }
    
    smoothScrollTo(element) {
        const offsetTop = element.offsetTop - 80; // Account for fixed navbar
        
        window.scrollTo({
            top: offsetTop,
            behavior: 'smooth'
        });
    }
    
    scrollToTop() {
        window.scrollTo({
            top: 0,
            behavior: 'smooth'
        });
    }
    
    setupIntersectionObserver() {
        const observerOptions = {
            threshold: 0.1,
            rootMargin: '-50px 0px'
        };
        
        const observer = new IntersectionObserver((entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    entry.target.classList.add('animate-in');
                    
                    // Trigger specific animations based on element type
                    if (entry.target.classList.contains('stat-number')) {
                        this.animateCounter(entry.target);
                    }
                }
            });
        }, observerOptions);
        
        // Observe elements for animation
        const animatedElements = document.querySelectorAll(
            '.feature-card, .ecosystem-card, .stat, .tech-panel'
        );
        
        animatedElements.forEach(el => observer.observe(el));
    }
    
    initializeCounters() {
        const counters = document.querySelectorAll('.stat-number');
        counters.forEach(counter => {
            counter.textContent = '0';
        });
    }
    
    animateCounters() {
        const counters = document.querySelectorAll('.stat-number');
        counters.forEach(counter => this.animateCounter(counter));
    }
    
    animateCounter(element) {
        const target = parseInt(element.getAttribute('data-target'));
        const duration = 2000;
        const step = target / (duration / 16);
        let current = 0;
        
        const updateCounter = () => {
            current += step;
            if (current < target) {
                element.textContent = Math.floor(current).toLocaleString();
                requestAnimationFrame(updateCounter);
            } else {
                element.textContent = target.toLocaleString();
            }
        };
        
        updateCounter();
    }
    
    initializeParticles() {
        const particlesContainer = document.querySelector('.floating-particles');
        if (!particlesContainer) return;
        
        const particleCount = 50;
        
        for (let i = 0; i < particleCount; i++) {
            const particle = document.createElement('div');
            particle.className = 'particle';
            particle.style.cssText = `
                position: absolute;
                width: ${Math.random() * 4 + 1}px;
                height: ${Math.random() * 4 + 1}px;
                background: rgba(102, 126, 234, ${Math.random() * 0.5 + 0.1});
                border-radius: 50%;
                left: ${Math.random() * 100}%;
                top: ${Math.random() * 100}%;
                animation: float ${Math.random() * 10 + 10}s infinite linear;
            `;
            
            particlesContainer.appendChild(particle);
        }
        
        // Add CSS animation for particles
        if (!document.getElementById('particle-styles')) {
            const style = document.createElement('style');
            style.id = 'particle-styles';
            style.textContent = `
                @keyframes float {
                    0% { transform: translateY(100vh) rotate(0deg); opacity: 0; }
                    10% { opacity: 1; }
                    90% { opacity: 1; }
                    100% { transform: translateY(-100px) rotate(360deg); opacity: 0; }
                }
            `;
            document.head.appendChild(style);
        }
    }
    
    initializeDAGVisualization() {
        const dagContainer = document.querySelector('.dag-network');
        if (!dagContainer) return;
        
        const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        svg.setAttribute('width', '100%');
        svg.setAttribute('height', '100%');
        svg.setAttribute('viewBox', '0 0 500 400');
        
        // Create DAG nodes and connections
        const nodes = [
            { id: 1, x: 100, y: 100, level: 0 },
            { id: 2, x: 200, y: 80, level: 1 },
            { id: 3, x: 200, y: 120, level: 1 },
            { id: 4, x: 300, y: 60, level: 2 },
            { id: 5, x: 300, y: 100, level: 2 },
            { id: 6, x: 300, y: 140, level: 2 },
            { id: 7, x: 400, y: 100, level: 3 }
        ];
        
        const connections = [
            { from: 1, to: 2 },
            { from: 1, to: 3 },
            { from: 2, to: 4 },
            { from: 2, to: 5 },
            { from: 3, to: 5 },
            { from: 3, to: 6 },
            { from: 4, to: 7 },
            { from: 5, to: 7 },
            { from: 6, to: 7 }
        ];
        
        // Create gradient definitions
        const defs = document.createElementNS('http://www.w3.org/2000/svg', 'defs');
        const gradient = document.createElementNS('http://www.w3.org/2000/svg', 'linearGradient');
        gradient.setAttribute('id', 'nodeGradient');
        gradient.setAttribute('x1', '0%');
        gradient.setAttribute('y1', '0%');
        gradient.setAttribute('x2', '100%');
        gradient.setAttribute('y2', '100%');
        
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
        
        // Draw connections
        connections.forEach(conn => {
            const fromNode = nodes.find(n => n.id === conn.from);
            const toNode = nodes.find(n => n.id === conn.to);
            
            const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
            line.setAttribute('x1', fromNode.x);
            line.setAttribute('y1', fromNode.y);
            line.setAttribute('x2', toNode.x);
            line.setAttribute('y2', toNode.y);
            line.setAttribute('stroke', 'rgba(102, 126, 234, 0.3)');
            line.setAttribute('stroke-width', '2');
            line.style.animation = `drawLine 2s ease-in-out ${conn.from * 0.2}s both`;
            
            svg.appendChild(line);
        });
        
        // Draw nodes
        nodes.forEach((node, index) => {
            const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
            circle.setAttribute('cx', node.x);
            circle.setAttribute('cy', node.y);
            circle.setAttribute('r', '12');
            circle.setAttribute('fill', 'url(#nodeGradient)');
            circle.setAttribute('stroke', '#ffffff');
            circle.setAttribute('stroke-width', '2');
            circle.style.animation = `nodeAppear 0.5s ease-out ${index * 0.3}s both`;
            
            svg.appendChild(circle);
        });
        
        dagContainer.appendChild(svg);
        
        // Add CSS animations
        if (!document.getElementById('dag-styles')) {
            const style = document.createElement('style');
            style.id = 'dag-styles';
            style.textContent = `
                @keyframes drawLine {
                    0% { stroke-dasharray: 1000; stroke-dashoffset: 1000; }
                    100% { stroke-dasharray: 1000; stroke-dashoffset: 0; }
                }
                @keyframes nodeAppear {
                    0% { opacity: 0; transform: scale(0); }
                    100% { opacity: 1; transform: scale(1); }
                }
            `;
            document.head.appendChild(style);
        }
    }
    
    initializeTechVisualizations() {
        this.createArchitectureDiagram();
        this.createConsensusDiagram();
        this.createCryptoDiagram();
        this.createPerformanceChart();
    }
    
    createArchitectureDiagram() {
        const container = document.querySelector('.architecture-diagram');
        if (!container) return;
        
        container.innerHTML = `
            <div class="arch-layer" style="background: linear-gradient(135deg, #667eea, #764ba2); color: white; padding: 20px; border-radius: 10px; margin: 10px 0;">
                <h4>Application Layer</h4>
                <p>DApps, Wallets, APIs</p>
            </div>
            <div class="arch-layer" style="background: linear-gradient(135deg, #f093fb, #f5576c); color: white; padding: 20px; border-radius: 10px; margin: 10px 0;">
                <h4>Consensus Layer</h4>
                <p>Hybrid PoW + DPoS + PoSe</p>
            </div>
            <div class="arch-layer" style="background: linear-gradient(135deg, #4facfe, #00f2fe); color: white; padding: 20px; border-radius: 10px; margin: 10px 0;">
                <h4>Network Layer</h4>
                <p>P2P Communication, DAG</p>
            </div>
            <div class="arch-layer" style="background: linear-gradient(135deg, #43e97b, #38f9d7); color: white; padding: 20px; border-radius: 10px; margin: 10px 0;">
                <h4>Storage Layer</h4>
                <p>Distributed Ledger, UTXO</p>
            </div>
        `;
    }
    
    createConsensusDiagram() {
        const container = document.querySelector('.consensus-diagram');
        if (!container) return;
        
        container.innerHTML = `
            <div style="display: flex; justify-content: space-around; align-items: center; height: 100%;">
                <div class="consensus-step" style="text-align: center; padding: 20px;">
                    <div style="width: 80px; height: 80px; background: linear-gradient(135deg, #667eea, #764ba2); border-radius: 50%; display: flex; align-items: center; justify-content: center; color: white; font-weight: bold; margin: 0 auto 10px;">PoW</div>
                    <p>Block Creation</p>
                </div>
                <div style="font-size: 24px; color: #667eea;">→</div>
                <div class="consensus-step" style="text-align: center; padding: 20px;">
                    <div style="width: 80px; height: 80px; background: linear-gradient(135deg, #f093fb, #f5576c); border-radius: 50%; display: flex; align-items: center; justify-content: center; color: white; font-weight: bold; margin: 0 auto 10px;">DPoS</div>
                    <p>Validation</p>
                </div>
                <div style="font-size: 24px; color: #667eea;">→</div>
                <div class="consensus-step" style="text-align: center; padding: 20px;">
                    <div style="width: 80px; height: 80px; background: linear-gradient(135deg, #4facfe, #00f2fe); border-radius: 50%; display: flex; align-items: center; justify-content: center; color: white; font-weight: bold; margin: 0 auto 10px;">PoSe</div>
                    <p>Finalization</p>
                </div>
            </div>
        `;
    }
    
    createCryptoDiagram() {
        const container = document.querySelector('.crypto-diagram');
        if (!container) return;
        
        container.innerHTML = `
            <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 20px; height: 100%;">
                <div style="background: linear-gradient(135deg, #667eea, #764ba2); color: white; padding: 20px; border-radius: 10px; text-align: center;">
                    <h4>CRYSTALS-Dilithium</h4>
                    <p>Digital Signatures</p>
                </div>
                <div style="background: linear-gradient(135deg, #f093fb, #f5576c); color: white; padding: 20px; border-radius: 10px; text-align: center;">
                    <h4>Kyber</h4>
                    <p>Key Encapsulation</p>
                </div>
                <div style="background: linear-gradient(135deg, #4facfe, #00f2fe); color: white; padding: 20px; border-radius: 10px; text-align: center;">
                    <h4>SPHINCS+</h4>
                    <p>Hash-based Signatures</p>
                </div>
                <div style="background: linear-gradient(135deg, #43e97b, #38f9d7); color: white; padding: 20px; border-radius: 10px; text-align: center;">
                    <h4>Homomorphic</h4>
                    <p>Encrypted Computation</p>
                </div>
            </div>
        `;
    }
    
    createPerformanceChart() {
        const container = document.querySelector('.performance-chart');
        if (!container) return;
        
        const canvas = document.createElement('canvas');
        canvas.width = 400;
        canvas.height = 300;
        container.appendChild(canvas);
        
        const ctx = canvas.getContext('2d');
        
        // Simple bar chart
        const data = [
            { label: 'TPS', value: 100000, color: '#667eea' },
            { label: 'Finality (ms)', value: 500, color: '#764ba2' },
            { label: 'Nodes', value: 1000, color: '#f093fb' },
            { label: 'Security', value: 256, color: '#4facfe' }
        ];
        
        const maxValue = Math.max(...data.map(d => d.value));
        const barWidth = 60;
        const barSpacing = 80;
        const chartHeight = 200;
        
        data.forEach((item, index) => {
            const barHeight = (item.value / maxValue) * chartHeight;
            const x = 50 + index * barSpacing;
            const y = 250 - barHeight;
            
            // Draw bar
            ctx.fillStyle = item.color;
            ctx.fillRect(x, y, barWidth, barHeight);
            
            // Draw label
            ctx.fillStyle = '#333';
            ctx.font = '12px Inter';
            ctx.textAlign = 'center';
            ctx.fillText(item.label, x + barWidth / 2, 270);
            
            // Draw value
            ctx.fillStyle = '#666';
            ctx.font = '10px Inter';
            ctx.fillText(item.value.toLocaleString(), x + barWidth / 2, y - 5);
        });
    }
    
    switchTechTab(e) {
        const clickedTab = e.target;
        const tabId = clickedTab.getAttribute('data-tab');
        
        // Update tab states
        document.querySelectorAll('.tech-tab').forEach(tab => {
            tab.classList.remove('active');
        });
        clickedTab.classList.add('active');
        
        // Update panel states
        document.querySelectorAll('.tech-panel').forEach(panel => {
            panel.classList.remove('active');
        });
        
        const targetPanel = document.getElementById(tabId);
        if (targetPanel) {
            targetPanel.classList.add('active');
        }
    }
    
    initializeCodeHighlighting() {
        // Simple syntax highlighting for Rust code
        const codeBlocks = document.querySelectorAll('code.language-rust');
        codeBlocks.forEach(block => {
            let html = block.innerHTML;
            
            // Keywords
            html = html.replace(/\b(use|async|fn|let|mut|await|Result|Ok|println!)\b/g, '<span style="color: #c678dd;">$1</span>');
            
            // Strings
            html = html.replace(/"([^"]*)"/g, '<span style="color: #98c379;">"$1"</span>');
            
            // Comments
            html = html.replace(/\/\/([^\n]*)/g, '<span style="color: #5c6370;">// $1</span>');
            
            // Functions
            html = html.replace(/\b([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/g, '<span style="color: #61afef;">$1</span>(');
            
            block.innerHTML = html;
        });
    }
    
    handleParallaxEffects() {
        const parallaxElements = document.querySelectorAll('.quantum-grid');
        parallaxElements.forEach(element => {
            const speed = 0.5;
            const yPos = -(this.scrollPosition * speed);
            element.style.transform = `translateY(${yPos}px)`;
        });
    }
    
    resizeVisualizations() {
        // Recalculate and redraw visualizations on resize
        this.initializeDAGVisualization();
        this.createPerformanceChart();
    }
    
    initializeAnimations() {
        // Add entrance animations to elements
        const animatedElements = document.querySelectorAll(
            '.feature-card, .ecosystem-card, .tech-panel'
        );
        
        animatedElements.forEach((element, index) => {
            element.style.opacity = '0';
            element.style.transform = 'translateY(30px)';
            element.style.transition = 'opacity 0.6s ease, transform 0.6s ease';
            element.style.transitionDelay = `${index * 0.1}s`;
        });
    }
    
    handleFormSubmit(e) {
        e.preventDefault();
        
        const form = e.target;
        const formData = new FormData(form);
        const data = Object.fromEntries(formData.entries());
        
        // Show loading state
        const submitButton = form.querySelector('button[type="submit"]');
        const originalText = submitButton.textContent;
        submitButton.textContent = 'Submitting...';
        submitButton.disabled = true;
        
        // Simulate form submission
        setTimeout(() => {
            this.showNotification('Thank you for your submission!', 'success');
            form.reset();
            submitButton.textContent = originalText;
            submitButton.disabled = false;
        }, 2000);
    }
    
    showNotification(message, type = 'info') {
        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            background: ${type === 'success' ? '#48bb78' : type === 'error' ? '#f56565' : '#667eea'};
            color: white;
            padding: 16px 24px;
            border-radius: 8px;
            box-shadow: 0 10px 25px rgba(0, 0, 0, 0.1);
            z-index: 10000;
            transform: translateX(100%);
            transition: transform 0.3s ease;
        `;
        notification.textContent = message;
        
        document.body.appendChild(notification);
        
        // Animate in
        setTimeout(() => {
            notification.style.transform = 'translateX(0)';
        }, 100);
        
        // Auto remove
        setTimeout(() => {
            notification.style.transform = 'translateX(100%)';
            setTimeout(() => {
                document.body.removeChild(notification);
            }, 300);
        }, 5000);
    }
    
    // Utility methods
    debounce(func, wait) {
        let timeout;
        return function executedFunction(...args) {
            const later = () => {
                clearTimeout(timeout);
                func(...args);
            };
            clearTimeout(timeout);
            timeout = setTimeout(later, wait);
        };
    }
    
    throttle(func, limit) {
        let inThrottle;
        return function() {
            const args = arguments;
            const context = this;
            if (!inThrottle) {
                func.apply(context, args);
                inThrottle = true;
                setTimeout(() => inThrottle = false, limit);
            }
        };
    }
}

// Initialize the website when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.qantoWebsite = new QantoWebsite();
});

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = QantoWebsite;
}