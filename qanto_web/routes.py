from flask import Blueprint, render_template_string
from .extensions import db
from .models import Question

main_bp = Blueprint('main', __name__)

@main_bp.route('/')
def index():
    """
    This is the main route for the application.
    It queries the database for all questions and displays the count.
    """
    try:
        # This is a more robust way to query the database that ensures
        # the application context is handled correctly.
        questions = db.session.execute(db.select(Question)).scalars().all()
        question_count = len(questions)
        
        # Using a simple template string to display the result
        template = """
        <!DOCTYPE html>
        <html>
        <head>
            <title>Qanto</title>
            <style>
                body { font-family: sans-serif; background-color: #f4f4f9; color: #333; text-align: center; margin-top: 50px; }
                h1 { color: #4a4a8a; }
            </style>
        </head>
        <body>
            <h1>Welcome to Qanto!</h1>
            <p>The database has been initialized successfully.</p>
            <p>So far, you have <strong>{{ count }}</strong> questions.</p>
        </body>
        </html>
        """
        return render_template_string(template, count=question_count)

    except Exception as e:
        # This will catch any database errors and display them if the app is in debug mode.
        return f"<h1>An error occurred:</h1><p>{e}</p>", 500
