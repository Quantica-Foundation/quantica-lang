// src/doc_generator.rs

use crate::parser::ast::{ASTNode, Parameter, Type};
use std::fs;
use std::io::Write;
use std::path::Path;


pub struct DocGenerator {
    markdown_buffer: String,
}

impl DocGenerator {
  
    pub fn new() -> Self {
        DocGenerator {
            markdown_buffer: String::new(),
        }
    }

   
    pub fn run(ast: &ASTNode, output_file: &str) -> Result<(), String> {
        let mut generator = Self::new();
        generator.walk_program(ast)?;
        
    
        if let Some(parent_dir) = Path::new(output_file).parent() {
            if !parent_dir.exists() {
                fs::create_dir_all(parent_dir)
                    .map_err(|e| format!("Failed to create docs directory: {}", e))?;
            }
        }
        
     
        let mut file = fs::File::create(output_file)
            .map_err(|e| format!("Failed to create doc file: {}", e))?;
        
        file.write_all(generator.markdown_buffer.as_bytes())
            .map_err(|e| format!("Failed to write to doc file: {}", e))?;
            
        Ok(())
    }

   
    fn walk_program(&mut self, node: &ASTNode) -> Result<(), String> {
        if let ASTNode::Program(statements) = node {
       
            self.markdown_buffer.push_str("# Quantica API Reference\n\n");
            
            for stmt in statements {
                self.generate_doc_for_node(stmt);
            }
            Ok(())
        } else {
            Err("Expected Program node".to_string())
        }
    }

   
    fn generate_doc_for_node(&mut self, node: &ASTNode) {
        match node {
            ASTNode::FunctionDeclaration { name, parameters, return_type, .. } => {
                
                self.add_entry(
                    &format!("func {}", name),
                    &self.format_params(parameters),
                    return_type,
                    "No documentation available.",
                );
            }
            ASTNode::CircuitDeclaration { name, parameters, return_type, .. } => {
                self.add_entry(
                    &format!("circuit {}", name),
                    &self.format_params(parameters),
                    return_type,
                    "No documentation available.",
                );
            }
            ASTNode::LetDeclaration { name, type_annotation, is_mutable, .. } => {
                let prefix = if *is_mutable { "mut" } else { "let" };
                self.add_variable_entry(
                    &format!("{} {}", prefix, name),
                    type_annotation,
                    "No documentation available.",
                );
            }
            _ => {
                
            }
        }
    }


    fn add_entry(
        &mut self,
        name: &str,
        params_str: &str,
        return_type: &Option<Type>,
        comment: &str,
    ) {

        self.markdown_buffer.push_str(&format!(
            "### `{}({}){}`\n\n",
            name,
            params_str,
            self.format_return_type(return_type)
        ));
        
 
        self.markdown_buffer.push_str(comment);
        self.markdown_buffer.push_str("\n\n---\n\n");
    }
    
   
    fn add_variable_entry(
        &mut self,
        name: &str,
        type_annotation: &Option<Type>,
        comment: &str,
    ) {
        
        let type_str = if let Some(t) = type_annotation {
            format!(": {}", self.format_type(t))
        } else {
            "".to_string()
        };
        self.markdown_buffer.push_str(&format!(
            "### `{}`\n\n",
            format!("{}{}", name, type_str)
        ));
        
     
        self.markdown_buffer.push_str(comment);
        self.markdown_buffer.push_str("\n\n---\n\n");
    }

 
    fn format_params(&self, params: &[Parameter]) -> String {
        params.iter()
            .map(|p| format!("{}: {}", p.name, self.format_type(&p.param_type)))
            .collect::<Vec<String>>()
            .join(", ")
    }


    fn format_return_type(&self, return_type: &Option<Type>) -> String {
        if let Some(t) = return_type {
            format!(" -> {}", self.format_type(t))
        } else {
            "".to_string()
        }
    }
    

    fn format_type(&self, t: &Type) -> String {
       
        format!("{:?}", t)
    }

}

