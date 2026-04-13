// Qanto Website - Advanced Animations

class QantoAnimations {
    constructor() {
        this.animationQueue = [];
        this.isAnimating = false;
        this.observers = new Map();
        
        this.init();
    }
    
    init() {
        this.setupIntersectionObservers();
        this.initializeParticleSystem();
        this.setupScrollAnimations();
        this.initializeTypewriter();
        this.setupHoverEffects();
    }
    
    setupIntersectionObservers() {
        // Fade in animation observer
        const fadeObserver = new IntersectionObserver((entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    this.animateFadeIn(entry.target);
                }
            });
        }, {
            threshold: 0.1,
            rootMargin: '0px 0px -50px 0px'
        });
        
        // Slide up animation observer
        const slideObserver = new IntersectionObserver((entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    this.animateSlideUp(entry.target);
                }
            });
        }, {
            threshold: 0.2,
            rootMargin: '0px 0px -100px 0px'
        });
        
        // Scale animation observer
        const scaleObserver = new IntersectionObserver((entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    this.animateScale(entry.target);
                }
            });
        }, {
            threshold: 0.3
        });
        
        // Apply observers to elements
        document.querySelectorAll('.animate-fade').forEach(el => {
            fadeObserver.observe(el);
        });
        
        document.querySelectorAll('.animate-slide').forEach(el => {
            slideObserver.observe(el);
        });
        
        document.querySelectorAll('.animate-scale').forEach(el => {
            scaleObserver.observe(el);
        });
        
        this.observers.set('fade', fadeObserver);
        this.observers.set('slide', slideObserver);
        this.observers.set('scale', scaleObserver);
    }
    
    animateFadeIn(element) {
        if (element.classList.contains('animated')) return;
        
        element.style.opacity = '0';
        element.style.transition = 'opacity 0.8s ease-out';
        
        requestAnimationFrame(() => {
            element.style.opacity = '1';
            element.classList.add('animated');
        });
    }
    
    animateSlideUp(element) {
        if (element.classList.contains('animated')) return;
        
        element.style.transform = 'translateY(50px)';
        element.style.opacity = '0';
        element.style.transition = 'transform 0.6s ease-out, opacity 0.6s ease-out';
        
        requestAnimationFrame(() => {
            element.style.transform = 'translateY(0)';
            element.style.opacity = '1';
            element.classList.add('animated');
        });
    }
    
    animateScale(element) {
        if (element.classList.contains('animated')) return;
        
        element.style.transform = 'scale(0.8)';
        element.style.opacity = '0';
        element.style.transition = 'transform 0.5s ease-out, opacity 0.5s ease-out';
        
        requestAnimationFrame(() => {
            element.style.transform = 'scale(1)';
            element.style.opacity = '1';
            element.classList.add('animated');
        });
    }
    
    initializeParticleSystem() {
        const canvas = document.createElement('canvas');
        canvas.id = 'particle-canvas';
        canvas.style.cssText = `
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            pointer-events: none;
            z-index: -1;
            opacity: 0.6;
        `;
        
        document.body.appendChild(canvas);
        
        const ctx = canvas.getContext('2d');
        const particles = [];
        const particleCount = 50;
        
        // Resize canvas
        const resizeCanvas = () => {
            canvas.width = window.innerWidth;
            canvas.height = window.innerHeight;
        };
        
        resizeCanvas();
        window.addEventListener('resize', resizeCanvas);
        
        // Particle class
        class Particle {
            constructor() {
                this.reset();
                this.y = Math.random() * canvas.height;
            }
            
            reset() {
                this.x = Math.random() * canvas.width;
                this.y = -10;
                this.size = Math.random() * 3 + 1;
                this.speed = Math.random() * 2 + 0.5;
                this.opacity = Math.random() * 0.5 + 0.2;
                this.color = `rgba(102, 126, 234, ${this.opacity})`;
            }
            
            update() {
                this.y += this.speed;
                
                if (this.y > canvas.height + 10) {
                    this.reset();
                }
            }
            
            draw() {
                ctx.beginPath();
                ctx.arc(this.x, this.y, this.size, 0, Math.PI * 2);
                ctx.fillStyle = this.color;
                ctx.fill();
            }
        }
        
        // Create particles
        for (let i = 0; i < particleCount; i++) {
            particles.push(new Particle());
        }
        
        // Animation loop
        const animate = () => {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            
            particles.forEach(particle => {
                particle.update();
                particle.draw();
            });
            
            requestAnimationFrame(animate);
        };
        
        animate();
    }
    
    setupScrollAnimations() {
        let ticking = false;
        
        const updateScrollAnimations = () => {
            const scrolled = window.pageYOffset;
            const rate = scrolled * -0.5;
            
            // Parallax effect for hero background
            const hero = document.querySelector('.hero');
            if (hero) {
                hero.style.transform = `translateY(${rate}px)`;
            }
            
            // Floating elements
            const floatingElements = document.querySelectorAll('.floating-element');
            floatingElements.forEach((element, index) => {
                const speed = 0.2 + (index * 0.1);
                const yPos = scrolled * speed;
                element.style.transform = `translateY(${yPos}px)`;
            });
            
            ticking = false;
        };
        
        const requestScrollUpdate = () => {
            if (!ticking) {
                requestAnimationFrame(updateScrollAnimations);
                ticking = true;
            }
        };
        
        window.addEventListener('scroll', requestScrollUpdate);
    }
    
    initializeTypewriter() {
        const typewriterElements = document.querySelectorAll('.typewriter');
        
        typewriterElements.forEach(element => {
            const text = element.textContent;
            const speed = parseInt(element.dataset.speed) || 50;
            const delay = parseInt(element.dataset.delay) || 0;
            
            element.textContent = '';
            element.style.borderRight = '2px solid #667eea';
            element.style.animation = 'blink 1s infinite';
            
            setTimeout(() => {
                this.typeText(element, text, speed);
            }, delay);
        });
        
        // Add blinking cursor animation
        if (!document.getElementById('typewriter-styles')) {
            const style = document.createElement('style');
            style.id = 'typewriter-styles';
            style.textContent = `
                @keyframes blink {
                    0%, 50% { border-color: transparent; }
                    51%, 100% { border-color: #667eea; }
                }
            `;
            document.head.appendChild(style);
        }
    }
    
    typeText(element, text, speed) {
        let i = 0;
        const timer = setInterval(() => {
            element.textContent += text.charAt(i);
            i++;
            
            if (i >= text.length) {
                clearInterval(timer);
                // Remove cursor after typing is complete
                setTimeout(() => {
                    element.style.borderRight = 'none';
                }, 1000);
            }
        }, speed);
    }
    
    setupHoverEffects() {
        // Card hover effects
        const cards = document.querySelectorAll('.feature-card, .ecosystem-card');
        cards.forEach(card => {
            card.addEventListener('mouseenter', () => {
                this.animateCardHover(card, true);
            });
            
            card.addEventListener('mouseleave', () => {
                this.animateCardHover(card, false);
            });
        });
        
        // Button hover effects
        const buttons = document.querySelectorAll('.btn-primary, .btn-secondary');
        buttons.forEach(button => {
            button.addEventListener('mouseenter', () => {
                this.animateButtonHover(button, true);
            });
            
            button.addEventListener('mouseleave', () => {
                this.animateButtonHover(button, false);
            });
        });
        
        // Navigation link effects
        const navLinks = document.querySelectorAll('.nav-link');
        navLinks.forEach(link => {
            link.addEventListener('mouseenter', () => {
                this.animateNavLinkHover(link, true);
            });
            
            link.addEventListener('mouseleave', () => {
                this.animateNavLinkHover(link, false);
            });
        });
    }
    
    animateCardHover(card, isHover) {
        const scale = isHover ? 'scale(1.05)' : 'scale(1)';
        const shadow = isHover ? '0 20px 40px rgba(0, 0, 0, 0.1)' : '0 10px 30px rgba(0, 0, 0, 0.05)';
        
        card.style.transform = scale;
        card.style.boxShadow = shadow;
        card.style.transition = 'transform 0.3s ease, box-shadow 0.3s ease';
    }
    
    animateButtonHover(button, isHover) {
        if (isHover) {
            button.style.transform = 'translateY(-2px)';
            button.style.boxShadow = '0 10px 20px rgba(102, 126, 234, 0.3)';
        } else {
            button.style.transform = 'translateY(0)';
            button.style.boxShadow = '0 4px 15px rgba(102, 126, 234, 0.2)';
        }
        
        button.style.transition = 'transform 0.2s ease, box-shadow 0.2s ease';
    }
    
    animateNavLinkHover(link, isHover) {
        if (isHover) {
            link.style.color = '#667eea';
            link.style.transform = 'translateY(-1px)';
        } else {
            link.style.color = '';
            link.style.transform = 'translateY(0)';
        }
        
        link.style.transition = 'color 0.2s ease, transform 0.2s ease';
    }
    
    // Utility animation methods
    fadeIn(element, duration = 300) {
        element.style.opacity = '0';
        element.style.display = 'block';
        element.style.transition = `opacity ${duration}ms ease`;
        
        requestAnimationFrame(() => {
            element.style.opacity = '1';
        });
    }
    
    fadeOut(element, duration = 300) {
        element.style.transition = `opacity ${duration}ms ease`;
        element.style.opacity = '0';
        
        setTimeout(() => {
            element.style.display = 'none';
        }, duration);
    }
    
    slideDown(element, duration = 300) {
        element.style.height = '0';
        element.style.overflow = 'hidden';
        element.style.transition = `height ${duration}ms ease`;
        element.style.display = 'block';
        
        const height = element.scrollHeight;
        
        requestAnimationFrame(() => {
            element.style.height = height + 'px';
        });
        
        setTimeout(() => {
            element.style.height = 'auto';
            element.style.overflow = 'visible';
        }, duration);
    }
    
    slideUp(element, duration = 300) {
        element.style.height = element.offsetHeight + 'px';
        element.style.overflow = 'hidden';
        element.style.transition = `height ${duration}ms ease`;
        
        requestAnimationFrame(() => {
            element.style.height = '0';
        });
        
        setTimeout(() => {
            element.style.display = 'none';
        }, duration);
    }
    
    pulse(element, duration = 1000) {
        element.style.animation = `pulse ${duration}ms ease-in-out`;
        
        // Add pulse keyframes if not exists
        if (!document.getElementById('pulse-styles')) {
            const style = document.createElement('style');
            style.id = 'pulse-styles';
            style.textContent = `
                @keyframes pulse {
                    0% { transform: scale(1); }
                    50% { transform: scale(1.05); }
                    100% { transform: scale(1); }
                }
            `;
            document.head.appendChild(style);
        }
        
        setTimeout(() => {
            element.style.animation = '';
        }, duration);
    }
    
    shake(element, duration = 500) {
        element.style.animation = `shake ${duration}ms ease-in-out`;
        
        // Add shake keyframes if not exists
        if (!document.getElementById('shake-styles')) {
            const style = document.createElement('style');
            style.id = 'shake-styles';
            style.textContent = `
                @keyframes shake {
                    0%, 100% { transform: translateX(0); }
                    10%, 30%, 50%, 70%, 90% { transform: translateX(-5px); }
                    20%, 40%, 60%, 80% { transform: translateX(5px); }
                }
            `;
            document.head.appendChild(style);
        }
        
        setTimeout(() => {
            element.style.animation = '';
        }, duration);
    }
    
    bounce(element, duration = 600) {
        element.style.animation = `bounce ${duration}ms ease`;
        
        // Add bounce keyframes if not exists
        if (!document.getElementById('bounce-styles')) {
            const style = document.createElement('style');
            style.id = 'bounce-styles';
            style.textContent = `
                @keyframes bounce {
                    0%, 20%, 53%, 80%, 100% {
                        transform: translate3d(0, 0, 0);
                    }
                    40%, 43% {
                        transform: translate3d(0, -20px, 0);
                    }
                    70% {
                        transform: translate3d(0, -10px, 0);
                    }
                    90% {
                        transform: translate3d(0, -4px, 0);
                    }
                }
            `;
            document.head.appendChild(style);
        }
        
        setTimeout(() => {
            element.style.animation = '';
        }, duration);
    }
    
    // Advanced animation sequences
    animateSequence(elements, animation, delay = 100) {
        elements.forEach((element, index) => {
            setTimeout(() => {
                this[animation](element);
            }, index * delay);
        });
    }
    
    // Cleanup method
    destroy() {
        this.observers.forEach(observer => {
            observer.disconnect();
        });
        
        const particleCanvas = document.getElementById('particle-canvas');
        if (particleCanvas) {
            particleCanvas.remove();
        }
    }
}

// Initialize animations when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.qantoAnimations = new QantoAnimations();
});

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = QantoAnimations;
}