from flask_wtf.csrf import CSRFProtect

csrf = CSRFProtect()

def init_app(app):
    csrf.init_app(app)
