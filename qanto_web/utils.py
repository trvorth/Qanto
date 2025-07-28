from sqlalchemy import func
from .models import db, Question

def get_random_question():
    return Question.query.order_by(func.random()).first()

